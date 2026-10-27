"""BAP unicast server of every emulated sink: PACS, ASCS and CAS.

Sink PAC: LC3 at 16 / 24 / 32 / 48 kHz, 10 ms (and 7.5 ms) frames, 26-155
octets, the channel counts and audio locations of the sink (sink.py), 1
codec frame per channel and SDU (Supported_Max_Codec_Frames_Per_SDU counts
per channel: a 2-channel ASE still takes 1). The
ASCS state machine is Bumble's; with a Sink ASE it accepts the CIS and goes
to Streaming by itself (Receiver Start Ready of the server). Advertising:
ASCS and CAS service data (general announcement, media available).

Release: Releasing, then Idle once the data path is gone (patch_release()
keeps the notifications in that order). --ascs-release-waits-for-cis: a
released ASE whose CIS is still up stays in Releasing until the client
disconnects the CIS (as many real servers do; Bumble's ASCS goes to Idle
by itself). --available-contexts-after S: no sink context available until
S seconds after the LE connection (then all, notified), so Enable has to
wait for it.
"""

import asyncio
import logging
import struct

from bumble import gatt, utils
from bumble.core import AdvertisingData
from bumble.profiles.ascs import (
    AseReasonCode,
    AseResponseCode,
    AseStateMachine,
    AudioStreamControlService,
)
from bumble.profiles.bap import (
    AudioLocation,
    CodecSpecificCapabilities,
    ContextType,
    SupportedFrameDuration,
    SupportedSamplingFrequency,
)
from bumble.profiles.pacs import PacRecord, PublishedAudioCapabilitiesService
from bumble import hci

log = logging.getLogger('emu_le_audio')

CONTEXTS = ContextType.MEDIA | ContextType.UNSPECIFIED | ContextType.CONVERSATIONAL | ContextType.GAME


def add_arguments(parser):
    parser.add_argument('--ascs-release-waits-for-cis', action='store_true',
                        help='keep a released ASE in Releasing until the client drops its CIS')
    parser.add_argument('--available-contexts-after', type=float, metavar='SECONDS',
                        help='no available sink context until SECONDS after the LE connection')


def patch_release(sink, ascs, ase, wait_for_cis):
    """Release of an ASE.

    Bumble's own handler goes to Idle in a task that, when the CIS is
    already gone, notifies Idle before the Releasing notification of the
    Release write itself (so the client is left seeing Releasing). Here the
    Idle transition comes after that notification.

    wait_for_cis: an ASE whose CIS is still up stays in Releasing until the
    client disconnects the CIS, then goes to Idle (no caching)."""
    original_disconnection = ase.on_cis_disconnection
    waiting = []

    async def idle_later():
        await asyncio.sleep(0.02)     # after the notifications of the Release write
        if ase.state == AseStateMachine.State.RELEASING:
            ase.state = AseStateMachine.State.IDLE
            await ascs.device.notify_subscribers(ase, ase.value)

    def on_release():
        if ase.state == AseStateMachine.State.IDLE:
            return (AseResponseCode.INVALID_ASE_STATE_MACHINE_TRANSITION, AseReasonCode.NONE)
        if ase.cis_link is not None and not wait_for_cis:
            # Bumble's: removes the data path first, so Idle comes later anyway
            return type(ase).on_release(ase)
        ase.state = AseStateMachine.State.RELEASING
        if ase.cis_link is not None:
            waiting.append(True)
            log.info('%s: ASE %d releasing, waiting for the client to drop the CIS', sink.name, ase.ase_id)
        else:
            utils.cancel_on_event(ascs.device, 'flush', idle_later())
        return (AseResponseCode.SUCCESS, AseReasonCode.NONE)

    def on_cis_disconnection(reason):
        original_disconnection(reason)
        # Also without wait_for_cis: when the client drops the CIS while
        # Bumble's release task removes the data path, that removal fails
        # (Unknown Connection Identifier) and the ASE would stay in Releasing
        if ase.state == AseStateMachine.State.RELEASING:
            if waiting:
                waiting.clear()
                log.info('%s: ASE %d released after the CIS was dropped', sink.name, ase.ase_id)
            utils.cancel_on_event(ascs.device, 'flush', idle_later())

    # ASCS looks the handlers up on the instance (getattr / bound at CIS set-up)
    ase.on_release = on_release
    ase.on_cis_disconnection = on_cis_disconnection


def configure_sink(sink, args):
    device = sink.device
    record = PacRecord(
        coding_format=hci.CodingFormat(hci.CodecID.LC3),
        codec_specific_capabilities=CodecSpecificCapabilities(
            supported_sampling_frequencies=(
                SupportedSamplingFrequency.FREQ_16000
                | SupportedSamplingFrequency.FREQ_24000
                | SupportedSamplingFrequency.FREQ_32000
                | SupportedSamplingFrequency.FREQ_48000
            ),
            supported_frame_durations=(
                SupportedFrameDuration.DURATION_10000_US_SUPPORTED
                | SupportedFrameDuration.DURATION_7500_US_SUPPORTED
            ),
            supported_audio_channel_count=sink.channel_counts,
            min_octets_per_codec_frame=26,
            max_octets_per_codec_frame=155,
            supported_max_codec_frames_per_sdu=1,
        ),
    )
    later = getattr(args, 'available_contexts_after', None) is not None
    pacs = PublishedAudioCapabilitiesService(
        supported_source_context=ContextType.PROHIBITED,
        supported_sink_context=CONTEXTS,
        available_source_context=ContextType.PROHIBITED,
        available_sink_context=ContextType.PROHIBITED if later else CONTEXTS,
        sink_pac=[record],
        sink_audio_locations=AudioLocation(sink.locations),
    )
    ascs = AudioStreamControlService(device, sink_ase_id=list(range(1, sink.sink_ases + 1)))
    cas = gatt.Service(gatt.GATT_COMMON_AUDIO_SERVICE, [])
    device.add_service(pacs)
    device.add_service(ascs)
    device.add_service(cas)
    sink.state['bap'] = dict(pacs=pacs, ascs=ascs)

    for ase in ascs.ase_state_machines.values():
        ase.on('state_change', lambda ase=ase: log.info('%s: ASE %d -> %s', sink.name, ase.ase_id, ase.state.name))
        patch_release(sink, ascs, ase, getattr(args, 'ascs_release_waits_for_cis', False))

    # ASCS: general announcement, available sink contexts, no metadata;
    # CAS: general announcement
    sink.adv_structures.append((AdvertisingData.SERVICE_DATA_16_BIT_UUID, struct.pack(
        '<HBHHB', 0x184E, 0x00, CONTEXTS, 0, 0)))
    sink.adv_structures.append((AdvertisingData.SERVICE_DATA_16_BIT_UUID, struct.pack(
        '<HB', 0x1853, 0x00)))


async def start(sink, args):
    log.info('%s: LE Audio sink, locations 0x%x, %d sink ASE(s), channel counts %s',
             sink.name, sink.locations, sink.sink_ases, sink.channel_counts)
    delay = getattr(args, 'available_contexts_after', None)
    if delay is not None:
        pacs = sink.state['bap']['pacs']
        device = sink.device

        async def make_available():
            await asyncio.sleep(delay)
            pacs.available_audio_contexts.value = struct.pack('<HH', CONTEXTS, 0)
            log.info('%s: available sink contexts now 0x%04x', sink.name, CONTEXTS)
            await device.notify_subscribers(pacs.available_audio_contexts)

        device.once('connection', lambda _connection: utils.AsyncRunner.spawn(make_available()))
