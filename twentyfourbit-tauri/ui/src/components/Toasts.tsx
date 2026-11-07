import type { ToastItem } from "../hooks/useToast";

export const Toasts = ({ items }: { items: ToastItem[] }) => (
  <div className="toasts" role="status" aria-live="polite">
    {items.map((t) => (
      <div
        key={t.id}
        className={`toast ${t.kind}${t.leaving ? " leaving" : ""}`}
      >
        {t.text}
      </div>
    ))}
  </div>
);
