import { createContext, useContext, useState, useCallback, ReactNode } from "react";
import { X, CheckCircle, AlertTriangle, Info } from "lucide-react";

interface Toast {
  id: string;
  type: "success" | "error" | "info";
  message: string;
}

interface ToastContextValue {
  showToast: (message: string, type?: "success" | "error" | "info") => void;
  success: (message: string) => void;
  error: (message: string) => void;
  info: (message: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within ToastProvider");
  return ctx;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showToast = useCallback((message: string, type: "success" | "error" | "info" = "info") => {
    const id = Math.random().toString(36).substring(7);
    setToasts((prev) => [...prev, { id, type, message }]);

    // Auto-dismiss after 5 seconds
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 5000);
  }, []);

  const success = useCallback((message: string) => showToast(message, "success"), [showToast]);
  const error = useCallback((message: string) => showToast(message, "error"), [showToast]);
  const info = useCallback((message: string) => showToast(message, "info"), [showToast]);

  const removeToast = (id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  };

  return (
    <ToastContext.Provider value={{ showToast, success, error, info }}>
      {children}
      <div className="fixed bottom-6 right-6 z-50 flex flex-col gap-2 pointer-events-none">
        {toasts.map((toast) => (
          <ToastMessage key={toast.id} toast={toast} onClose={() => removeToast(toast.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastMessage({ toast, onClose }: { toast: Toast; onClose: () => void }) {
  const icons = {
    success: <CheckCircle size={18} />,
    error: <AlertTriangle size={18} />,
    info: <Info size={18} />,
  };

  const colors = {
    success: {
      bg: "rgba(var(--primary), 0.15)",
      border: "rgba(var(--primary), 0.4)",
      text: "rgb(var(--primary))",
    },
    error: {
      bg: "rgba(var(--destructive), 0.15)",
      border: "rgba(var(--destructive), 0.4)",
      text: "rgb(var(--destructive))",
    },
    info: {
      bg: "rgba(var(--card), 0.9)",
      border: "rgba(var(--border), 0.4)",
      text: "rgb(var(--foreground))",
    },
  };

  return (
    <div
      className="pointer-events-auto flex items-center gap-3 px-4 py-3 rounded-lg border min-w-[300px] max-w-[400px] shadow-lg animate-in slide-in-from-right"
      style={{
        background: colors[toast.type].bg,
        borderColor: colors[toast.type].border,
        backdropFilter: "blur(20px)",
      }}
    >
      <div style={{ color: colors[toast.type].text }}>{icons[toast.type]}</div>
      <p className="flex-1 text-sm" style={{ color: colors[toast.type].text }}>
        {toast.message}
      </p>
      <button
        onClick={onClose}
        className="p-1 rounded hover:bg-black/10 transition-colors"
        style={{ color: colors[toast.type].text }}
      >
        <X size={14} />
      </button>
    </div>
  );
}
