import { Show } from "solid-js";
import { ICONS, type IconName } from "./icons";

/** A Fluent icon; decorative unless it has a title */
export function Icon(props: { name: IconName; class?: string; title?: string }) {
  return (
    <svg
      class={`icon ${props.class ?? ""}`}
      viewBox="0 0 20 20"
      fill="currentColor"
      aria-hidden={props.title ? undefined : "true"}
      role={props.title ? "img" : undefined}
    >
      <Show when={props.title}>
        <title>{props.title}</title>
      </Show>
      <path d={ICONS[props.name]} />
    </svg>
  );
}
