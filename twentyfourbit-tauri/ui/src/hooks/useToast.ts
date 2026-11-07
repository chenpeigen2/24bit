import { useCallback, useRef, useState } from "react";

export type ToastKind = "ok" | "err";
export interface ToastItem {
  id: number;
  text: string;
  kind: ToastKind;
  leaving: boolean;
}

/**
 * Transient messages.
 *
 * Bounded on purpose: a queue of ten toasts is noise. Each leaves on its own
 * timer, with a short exit transition so it does not blink out.
 */
export const useToast = () => {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(0);
  const timers = useRef<number[]>([]);

  const push = useCallback((text: string, kind: ToastKind = "ok") => {
    const id = nextId.current++;
    setItems((prev) => [...prev.slice(-2), { id, text, kind, leaving: false }]);

    const life = kind === "err" ? 6000 : 3200;
    timers.current.push(
      window.setTimeout(() => {
        setItems((prev) => prev.map((t) => (t.id === id ? { ...t, leaving: true } : t)));
        timers.current.push(
          window.setTimeout(() => {
            setItems((prev) => prev.filter((t) => t.id !== id));
          }, 240),
        );
      }, life),
    );
  }, []);

  return { toasts: items, push };
};
