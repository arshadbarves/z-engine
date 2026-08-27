/** Folded three-bar Z mark used in the splash, sidebar, and hero. */
export function LogoMark({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden
    >
      <rect x="1.5" y="1.5" width="21" height="21" rx="6" stroke="currentColor" strokeWidth="1.4" />
      <g fill="currentColor">
        <rect className="logo-bar logo-bar-top" x="5.53" y="5.53" width="12.94" height="3" rx="0.47" />
        <path className="logo-bar logo-bar-fold" d="M15.384 9.281h3.085L8.625 14.719H5.531z" />
        <rect className="logo-bar logo-bar-bot" x="5.53" y="15.47" width="12.94" height="3" rx="0.47" />
      </g>
    </svg>
  );
}
