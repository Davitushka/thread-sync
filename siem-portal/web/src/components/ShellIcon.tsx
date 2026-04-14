type Props = {
  iconKey: string;
  className?: string;
  size?: number;
};

const svgProps = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.7,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

function iconPath(iconKey: string) {
  switch (iconKey) {
    case "overview":
      return (
        <>
          <path {...svgProps} d="M4 18.5h16" />
          <path {...svgProps} d="M6.5 15.5V11" />
          <path {...svgProps} d="M12 15.5V7.5" />
          <path {...svgProps} d="M17.5 15.5V9.5" />
        </>
      );
    case "dashboards":
      return (
        <>
          <rect {...svgProps} x="4.5" y="4.5" width="6.5" height="6.5" rx="1.5" />
          <rect {...svgProps} x="13" y="4.5" width="6.5" height="6.5" rx="1.5" />
          <rect {...svgProps} x="4.5" y="13" width="6.5" height="6.5" rx="1.5" />
          <rect {...svgProps} x="13" y="13" width="6.5" height="6.5" rx="1.5" />
        </>
      );
    case "alerts":
      return (
        <>
          <path {...svgProps} d="M12 4.5 20 18.5H4L12 4.5Z" />
          <path {...svgProps} d="M12 9v4.5" />
          <path {...svgProps} d="M12 16.5h.01" />
        </>
      );
    case "detections":
      return (
        <>
          <circle {...svgProps} cx="12" cy="12" r="7.5" />
          <path {...svgProps} d="M12 4.5v3.2" />
          <path {...svgProps} d="M19.5 12h-3.2" />
          <path {...svgProps} d="M12 19.5v-3.2" />
          <path {...svgProps} d="m12 12 4-4" />
        </>
      );
    case "events":
      return (
        <>
          <rect {...svgProps} x="4.5" y="5" width="15" height="14" rx="2" />
          <path {...svgProps} d="M8 9h8" />
          <path {...svgProps} d="M8 12h8" />
          <path {...svgProps} d="M8 15h5" />
        </>
      );
    case "cases":
      return (
        <>
          <rect {...svgProps} x="4.5" y="6.5" width="15" height="11.5" rx="2" />
          <path {...svgProps} d="M9 6.5V5.2a1.2 1.2 0 0 1 1.2-1.2h3.6A1.2 1.2 0 0 1 15 5.2v1.3" />
          <path {...svgProps} d="M4.5 10.5h15" />
        </>
      );
    case "case-detail":
      return (
        <>
          <path {...svgProps} d="M7 4.5h7l3 3V19.5H7Z" />
          <path {...svgProps} d="M14 4.5v3h3" />
          <path {...svgProps} d="M9.5 11h5" />
          <path {...svgProps} d="M9.5 14h5" />
        </>
      );
    case "investigation":
      return (
        <>
          <circle {...svgProps} cx="10.5" cy="10.5" r="4.5" />
          <path {...svgProps} d="m14 14 5 5" />
          <path {...svgProps} d="M10.5 8.6v3.8" />
          <path {...svgProps} d="M8.6 10.5h3.8" />
        </>
      );
    case "infrastructure":
      return (
        <>
          <rect {...svgProps} x="5" y="5" width="14" height="5.5" rx="1.5" />
          <rect {...svgProps} x="5" y="13.5" width="14" height="5.5" rx="1.5" />
          <path {...svgProps} d="M8 8h.01" />
          <path {...svgProps} d="M8 16.5h.01" />
          <path {...svgProps} d="M11 8h5" />
          <path {...svgProps} d="M11 16.5h5" />
        </>
      );
    case "operations":
      return (
        <>
          <path {...svgProps} d="M4.5 14h3l2-5 4 8 2.5-5H19.5" />
          <path {...svgProps} d="M4.5 19.5h15" />
        </>
      );
    case "data-quality":
      return (
        <>
          <path {...svgProps} d="M12 4.5 18 7v5.5c0 3.3-2.2 5.7-6 7-3.8-1.3-6-3.7-6-7V7Z" />
          <path {...svgProps} d="m9.5 12 1.8 1.8L14.8 10" />
        </>
      );
    default:
      return (
        <>
          <rect {...svgProps} x="5" y="5" width="14" height="14" rx="2.5" />
          <path {...svgProps} d="M8.5 9.5h7" />
          <path {...svgProps} d="M8.5 13h7" />
        </>
      );
  }
}

export default function ShellIcon({ iconKey, className, size = 18 }: Props) {
  return (
    <span className={className} aria-hidden="true">
      <svg viewBox="0 0 24 24" width={size} height={size}>
        {iconPath(iconKey)}
      </svg>
    </span>
  );
}
