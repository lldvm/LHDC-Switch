// Form controls on native elements (keyboard and screen reader behaviour
// for free), styled in styles/app.css.

import {
  createEffect,
  createUniqueId,
  For,
  onCleanup,
  Show,
  splitProps,
  untrack,
  type JSX,
} from "solid-js";
import { Portal } from "solid-js/web";
import { Icon } from "./Icon";
import type { IconName } from "./icons";

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & {
  kind?: "default" | "subtle" | "danger" | "strong";
  icon?: IconName;
  busy?: boolean;
};

export function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ["kind", "icon", "busy", "children", "class", "disabled"]);
  return (
    <button
      type="button"
      class={`btn btn-${own.kind ?? "default"} ${own.class ?? ""}`}
      disabled={own.disabled || own.busy}
      aria-busy={own.busy ? "true" : undefined}
      {...rest}
    >
      <Show when={own.busy} fallback={own.icon && <Icon name={own.icon} />}>
        <span class="spinner" aria-hidden="true" />
      </Show>
      <Show when={own.children}>
        <span>{own.children}</span>
      </Show>
    </button>
  );
}

/** Icon-only button: the label is for screen readers and the tooltip */
export function IconButton(props: JSX.ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string }) {
  const [own, rest] = splitProps(props, ["icon", "label", "class"]);
  return (
    <button type="button" class={`btn btn-icon ${own.class ?? ""}`} aria-label={own.label} title={own.label} {...rest}>
      <Icon name={own.icon} />
    </button>
  );
}

export function Toggle(props: {
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  const id = createUniqueId();
  return (
    <div class="row toggle-row">
      <label class="row-text" for={id}>
        <span class="row-label">{props.label}</span>
        <Show when={props.hint}>
          <span class="hint" id={`${id}-hint`}>
            {props.hint}
          </span>
        </Show>
      </label>
      <input
        id={id}
        type="checkbox"
        role="switch"
        class="switch"
        checked={props.checked}
        disabled={props.disabled}
        aria-describedby={props.hint ? `${id}-hint` : undefined}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
    </div>
  );
}

export function Check(props: { label: string; checked: boolean; disabled?: boolean; title?: string; onChange: (v: boolean) => void }) {
  return (
    <label class="check" title={props.title}>
      <input
        type="checkbox"
        checked={props.checked}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
      <span>{props.label}</span>
    </label>
  );
}

export interface Option<T extends string | number> {
  value: T;
  label: string;
  icon?: IconName;
  disabled?: boolean;
}

export function Segmented<T extends string | number>(props: {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
}) {
  const name = createUniqueId();
  return (
    <div class="segmented" role="radiogroup" aria-label={props.label}>
      <For each={props.options}>
        {(o) => (
          <label class="seg" classList={{ on: o.value === props.value, disabled: o.disabled }}>
            <input
              type="radio"
              name={name}
              value={String(o.value)}
              checked={o.value === props.value}
              disabled={o.disabled}
              onChange={() => props.onChange(o.value)}
            />
            <Show when={o.icon}>{(i) => <Icon name={i()} />}</Show>
            <span>{o.label}</span>
          </label>
        )}
      </For>
    </div>
  );
}

/** A labelled control with an optional hint and error */
export function Field(props: {
  label: string;
  hint?: string;
  error?: string | null;
  children: (id: string, describedBy: string | undefined) => JSX.Element;
}) {
  const id = createUniqueId();
  // Built once: a reactive call would make a new input (and lose the focus
  // and the typed text) whenever the error comes or goes. References to an
  // absent hint or error are ignored by assistive technology.
  const control = untrack(() => props.children(id, `${id}-hint ${id}-err`));
  return (
    <div class="field" classList={{ invalid: !!props.error }}>
      <label class="field-label" for={id}>
        {props.label}
      </label>
      {control}
      <Show when={props.hint}>
        <span class="hint" id={`${id}-hint`}>
          {props.hint}
        </span>
      </Show>
      <Show when={props.error}>
        <span class="field-error" id={`${id}-err`} role="alert">
          <Icon name="error_circle" />
          {props.error}
        </span>
      </Show>
    </div>
  );
}

export function Select<T extends string | number>(props: {
  id?: string;
  value: T;
  options: Option<T>[];
  disabled?: boolean;
  label?: string;
  describedBy?: string;
  onChange: (v: T) => void;
}) {
  return (
    <div class="select">
      <select
        id={props.id}
        aria-label={props.label}
        aria-describedby={props.describedBy}
        disabled={props.disabled}
        onChange={(e) => {
          const o = props.options[e.currentTarget.selectedIndex];
          if (o) props.onChange(o.value);
        }}
      >
        <For each={props.options}>
          {(o) => (
            <option value={String(o.value)} selected={o.value === props.value} disabled={o.disabled}>
              {o.label}
            </option>
          )}
        </For>
      </select>
      <Icon name="chevron_down" class="select-chevron" />
    </div>
  );
}

/** A card section with a heading */
export function Section(props: { title: string; actions?: JSX.Element; children: JSX.Element; class?: string; id?: string }) {
  const id = createUniqueId();
  return (
    <section id={props.id} class={`card section ${props.class ?? ""}`} aria-labelledby={id}>
      <header class="section-head">
        <h2 id={id}>{props.title}</h2>
        <Show when={props.actions}>
          <div class="section-actions">{props.actions}</div>
        </Show>
      </header>
      {props.children}
    </section>
  );
}

export function Banner(props: {
  kind: "info" | "warn" | "error" | "good";
  title?: string;
  children?: JSX.Element;
  actions?: JSX.Element;
}) {
  const icon = (): IconName =>
    props.kind === "error" ? "error_circle" : props.kind === "warn" ? "warning" : props.kind === "good" ? "checkmark_circle" : "info";
  return (
    <div class={`banner banner-${props.kind}`} role={props.kind === "error" ? "alert" : "status"}>
      <Icon name={icon()} class="banner-icon" />
      <div class="banner-body">
        <Show when={props.title}>
          <strong class="banner-title">{props.title}</strong>
        </Show>
        <Show when={props.children}>
          <div class="banner-text">{props.children}</div>
        </Show>
        <Show when={props.actions}>
          <div class="banner-actions">{props.actions}</div>
        </Show>
      </div>
    </div>
  );
}

/** A modal question. Escape and the backdrop cancel; Tab stays inside. */
export function Dialog(props: {
  open: boolean;
  title: string;
  children: JSX.Element;
  confirm: string;
  cancel: string;
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const id = createUniqueId();
  let box: HTMLDivElement | undefined;
  let before: Element | null = null;
  createEffect(() => {
    if (!props.open) return;
    before = document.activeElement;
    queueMicrotask(() => box?.querySelector<HTMLElement>("[data-autofocus]")?.focus());
    onCleanup(() => {
      if (before instanceof HTMLElement) before.focus();
    });
  });
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      props.onCancel();
    } else if (e.key === "Tab" && box) {
      const f = [...box.querySelectorAll<HTMLElement>("button:not([disabled]),input,select,textarea,[tabindex='0']")];
      if (f.length === 0) return;
      const first = f[0];
      const last = f[f.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };
  return (
    <Show when={props.open}>
      <Portal>
        <div class="backdrop" onClick={(e) => e.target === e.currentTarget && props.onCancel()}>
          <div class="dialog card" role="dialog" aria-modal="true" aria-labelledby={id} ref={box} onKeyDown={onKey}>
            <h2 id={id}>{props.title}</h2>
            <div class="dialog-body">{props.children}</div>
            <div class="dialog-actions">
              <Button kind="subtle" onClick={() => props.onCancel()} data-autofocus>
                {props.cancel}
              </Button>
              <Button kind={props.danger ? "danger" : "strong"} busy={props.busy} onClick={() => props.onConfirm()}>
                {props.confirm}
              </Button>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
