// printf-style formatting of the translation strings (%s %d %u %zu %llu
// %.1f %+d %%), as the core's Msg::english does (crates/core/src/messages.rs).
// Arguments from the core arrive as strings and are inserted as they are;
// numbers from the UI itself get the precision, sign and integer
// conversion the placeholder asks for.

const SPEC = /%(%|([-+#0]*)(\d+)?(?:\.(\d+))?(?:hh|h|ll|l|z|j|t)?([diuxXfFeEgGcsp]))/g;

export function printf(template: string, args: readonly (string | number)[]): string {
  let next = 0;
  return template.replace(SPEC, (_m, whole: string, flags: string, width: string, prec: string, conv: string) => {
    if (whole === "%") return "%";
    const arg = args[next++];
    if (arg === undefined) return "";
    let s: string;
    if (typeof arg === "number") {
      switch (conv) {
        case "f":
        case "F":
          s = arg.toFixed(prec === undefined ? 6 : Number(prec));
          break;
        case "d":
        case "i":
        case "u":
          s = String(Math.trunc(arg));
          break;
        case "x":
          s = Math.trunc(arg).toString(16);
          break;
        case "X":
          s = Math.trunc(arg).toString(16).toUpperCase();
          break;
        default:
          s = String(arg);
      }
      if (flags.includes("+") && arg >= 0 && "dif".includes(conv)) s = "+" + s;
    } else {
      s = arg;
    }
    if (width && s.length < Number(width)) {
      s = flags.includes("-") ? s.padEnd(Number(width)) : s.padStart(Number(width), flags.includes("0") ? "0" : " ");
    }
    return s;
  });
}
