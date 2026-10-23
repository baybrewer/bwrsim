import { useEffect, useRef } from "react";
import type { RefObject } from "react";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.offsetParent !== null || el === document.activeElement);
}

/**
 * Modal focus handling: focuses into `ref` on mount, keeps Tab inside it, calls `onEscape`
 * for an Esc no inner handler claimed (preventDefault), and restores the prior focus on unmount.
 * Listens on the document so Esc works even if focus was lost to the body.
 */
export function useFocusTrap(ref: RefObject<HTMLElement>, onEscape: () => void): void {
  const escRef = useRef(onEscape);
  escRef.current = onEscape;
  // Captured at first render, before the page underneath goes inert (which blurs it).
  const priorRef = useRef(typeof document === "undefined" ? null : document.activeElement);

  useEffect(() => {
    const root = ref.current;
    if (!root) return;
    if (!root.contains(document.activeElement)) (focusables(root)[0] ?? root).focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (!e.defaultPrevented) {
          e.preventDefault();
          escRef.current();
        }
        return;
      }
      if (e.key !== "Tab") return;
      const items = focusables(root);
      if (items.length === 0) {
        e.preventDefault();
        root.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (!root.contains(active)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && (active === first || active === root)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      const prior = priorRef.current;
      if (prior instanceof HTMLElement && prior.isConnected) prior.focus();
    };
  }, [ref]);
}
