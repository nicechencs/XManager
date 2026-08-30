import type { ReactNode } from "react";
import type { PostKind } from "../types";
import { KIND_LABEL_ZH } from "../tweet";

export function Btn({
  variant = "ghost",
  disabled,
  onClick,
  children,
  title,
}: {
  variant?: "primary" | "ghost" | "danger";
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <button
      className={`btn ${variant}`}
      disabled={disabled}
      onClick={onClick}
      title={title}
    >
      {children}
    </button>
  );
}

export function Chip({
  active,
  onClick,
  children,
}: {
  active?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      className={`chip${active ? " active" : ""}`}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export function RemovableChip({
  label,
  onRemove,
}: {
  label: string;
  onRemove: () => void;
}) {
  return (
    <span className="removable-chip">
      {label}
      <button
        className="chip-remove"
        onClick={onRemove}
        aria-label={`移除筛选 ${label}`}
      >
        ×
      </button>
    </span>
  );
}

export function CountBadge({ n }: { n: number }) {
  if (n === 0) return null;
  return <span className="count-badge">{n}</span>;
}

export function StatusPill({
  healthy,
  label,
}: {
  healthy: boolean;
  label: string;
}) {
  return (
    <span className={`status-pill ${healthy ? "healthy" : "unhealthy"}`}>
      <span className="pill-dot">{healthy ? "●" : "○"}</span>
      {label}
    </span>
  );
}

export function SectionLabel({ children }: { children: ReactNode }) {
  return <div className="section-label">{children}</div>;
}

export function PageHeading({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle: string;
  actions?: ReactNode;
}) {
  return (
    <div className="page-heading">
      <div>
        <div className="heading-title">{title}</div>
        <div className="heading-subtitle">{subtitle}</div>
      </div>
      {actions ? <div className="heading-actions">{actions}</div> : null}
    </div>
  );
}

export function KindBadge({ kind }: { kind: PostKind }) {
  return <span className={`kind-badge ${kind}`}>{KIND_LABEL_ZH[kind]}</span>;
}

export function CheckboxMark({ checked }: { checked: boolean }) {
  return (
    <span className={`checkbox-mark${checked ? " checked" : ""}`}>
      {checked ? "✓" : ""}
    </span>
  );
}

/** − / value / + control; 0 renders as 不限 like the GPUI stepper. */
export function Stepper({
  value,
  min,
  max,
  step,
  onChange,
}: {
  value: number | null;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  const current = value ?? 0;
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  return (
    <span className="stepper">
      <button onClick={() => onChange(clamp(current - step))}>−</button>
      <span className="stepper-value" style={{ width: 64 }}>
        {current === 0 ? "不限" : current}
      </span>
      <button onClick={() => onChange(clamp(current + step))}>+</button>
      <span className="caption">
        [{min}–{max}] · 0 为不限
      </span>
    </span>
  );
}

export function EmptyMark({ glyph }: { glyph: string }) {
  return <div className="empty-mark">{glyph}</div>;
}

export function formatNumber(n: number): string {
  return n.toLocaleString("zh-CN");
}
