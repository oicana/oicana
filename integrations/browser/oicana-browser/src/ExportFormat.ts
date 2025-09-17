export type ExportFormat =
  | { format: 'pdf' | 'svg' }
  | { format: 'png'; pixelsPerPt: number };

export const Pdf: ExportFormat = { format: 'pdf' };
export const Svg: ExportFormat = { format: 'svg' };
export const Png = (pixelsPerPt: number): ExportFormat => ({
  format: 'png',
  pixelsPerPt,
});
