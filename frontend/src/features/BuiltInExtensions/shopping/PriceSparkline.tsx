import type { PricePoint } from "@type/browser";

export function PriceSparkline({ id, history, width = 120, height = 36 }: { id: string; history: PricePoint[]; width?: number; height?: number }) {
  if (history.length === 0) {
    return null;
  }
  const prices = history.map((point) => point.price);
  const min = Math.min(...prices);
  const max = Math.max(...prices);
  const points = history
    .map((point, index) => {
      const x = history.length === 1 ? width / 2 : (index / (history.length - 1)) * (width - 2) + 1;
      const y = max === min ? height / 2 : height - 4 - ((point.price - min) / (max - min)) * (height - 8);
      return `${x},${y}`;
    })
    .join(" ");
  return (
    <svg id={id} width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden className="text-emerald-300">
      <polyline id={`${id}-line`} points={points} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}
