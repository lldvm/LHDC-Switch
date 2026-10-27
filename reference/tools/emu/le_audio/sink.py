"""One emulated LE Audio sink (a headset, or one earbud of a pair).

SinkContext carries what the profile modules need while setting the sink up
(le_audio/__init__.py): the Bumble Device, its index and address, the audio
locations it renders, the AD structures to advertise and a dict for the
modules' own state.

With privacy (--privacy) a sink advertises and is connected with a
resolvable private address made from its IRK; pairing distributes the IRK
and the public address as identity, and the sink takes a new RPA whenever a
link went down (as real earbuds do every ~15 minutes).
"""

import asyncio
import dataclasses
import logging

from bumble import hci
from bumble.core import AdvertisingData
from bumble.device import Device
from bumble.host import Host
from bumble.keys import MemoryKeyStore
from bumble.pairing import PairingConfig, PairingDelegate
from bumble.transport.common import AsyncPipeSink

from .controller import LeEmuController, PacketLoggerSnooper

log = logging.getLogger('emu_le_audio')


@dataclasses.dataclass
class SinkContext:
    index: int                       # 0, 1, ...
    address: str                     # public LE address "00:A2:D0:00:00:12"
    name: str
    locations: int                   # Sink Audio Locations (bap.AudioLocation bits)
    sink_ases: int                   # number of sink ASEs
    channel_counts: list             # PAC Supported Audio Channel Counts
    device: Device = None
    adv_structures: list = dataclasses.field(default_factory=list)  # (type, bytes)
    state: dict = dataclasses.field(default_factory=dict)            # per module
    privacy: bool = False            # advertise / connect with an RPA (IRK distributed)


def create_device(link, sink, capture):
    """Controller + host + Device of the sink (not powered on yet)."""
    controller = LeEmuController(sink.name, link=link, public_address=sink.address)
    host = Host(controller, AsyncPipeSink(controller))
    if capture:
        host.snooper = PacketLoggerSnooper(capture)
    device = Device(name=sink.name, address=sink.address, host=host)
    device.le_enabled = True
    device.classic_enabled = False
    device.cis_enabled = True
    device.keystore = MemoryKeyStore()
    if sink.privacy:
        # A fixed IRK per sink; the initial RPA is made at power on, no
        # periodic change (rotate_rpa() does it after a link went down)
        device.irk = bytes([0xA0 + sink.index] * 16)
        device.le_privacy_enabled = True
        device.le_rpa_timeout = 0
    # LE Secure Connections, Just Works, bonding (as headsets without a display);
    # the public address is the identity distributed with the IRK
    device.pairing_config_factory = lambda connection: PairingConfig(
        sc=True, mitm=False, bonding=True, identity_address_type=hci.Address.PUBLIC_DEVICE_ADDRESS,
        delegate=PairingDelegate(io_capability=PairingDelegate.IoCapability.NO_OUTPUT_NO_INPUT))
    sink.device = device
    return device


def advertising_data(sink):
    """Flags, the modules' AD structures and the name in what is left of the
    31 bytes of legacy advertising (shortened if needed). Bumble's virtual
    controller sends the advertising data as scan response too, so there is
    no extra room there."""
    structures = [(AdvertisingData.FLAGS, bytes([0x06]))] + sink.adv_structures
    data = bytes(AdvertisingData(structures))
    room = 31 - len(data) - 2
    name = sink.name.encode()
    if room >= len(name):
        data += bytes(AdvertisingData([(AdvertisingData.COMPLETE_LOCAL_NAME, name)]))
    elif room >= 3:
        data += bytes(AdvertisingData([(AdvertisingData.SHORTENED_LOCAL_NAME, name[:room])]))
    if len(data) > 31:
        log.warning('%s: advertising data is %d bytes, legacy advertising takes 31', sink.name, len(data))
    return data


def current_address(sink):
    """The address the sink advertises now (its RPA with privacy)."""
    return str(sink.device.random_address if sink.privacy else sink.address).split('/')[0]


async def rotate_rpa(sink):
    """Privacy: a new RPA, advertised from now on."""
    device = sink.device
    await device.stop_advertising()
    rpa = hci.Address.generate_private_address(device.irk)
    await device.send_command(hci.HCI_LE_Set_Random_Address_Command(random_address=rpa))
    device.random_address = rpa
    log.info('%s: new RPA %s', sink.name, current_address(sink))
    await advertise(sink)


async def advertise(sink):
    await sink.device.start_advertising(
        own_address_type=hci.OwnAddressType.RANDOM if sink.privacy else hci.OwnAddressType.PUBLIC,
        advertising_data=advertising_data(sink),
        # With privacy the disconnection handler restarts it with a new RPA
        auto_restart=not sink.privacy,
    )


async def power_on_and_advertise(sink):
    device = sink.device
    await device.power_on()
    await advertise(sink)
    if sink.privacy:
        log.info('%s: privacy on, RPA %s, identity %s', sink.name, current_address(sink), sink.address)

        def on_connection(connection):
            connection.on('disconnection',
                          lambda reason: asyncio.get_running_loop().create_task(rotate_rpa(sink)))
        device.on('connection', on_connection)
    device.on('connection', lambda c: log.info('%s: LE connection from %s to %s', sink.name, c.peer_address,
                                               c.self_address))
