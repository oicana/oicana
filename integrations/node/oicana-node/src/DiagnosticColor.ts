import {
  DiagnosticColor as NativeDiagnosticColor,
  configureDiagnosticColor as nativeConfigureDiagnosticColor,
} from '@oicana/node-native';

/** Color mode for compilation diagnostics. */
export enum DiagnosticColor {
  None = 'none',
  Ansi = 'ansi',
}

/**
 * Configure the coloring of compilation diagnostics like warnings and errors.
 */
export function configureDiagnosticColor(color: DiagnosticColor): void {
  nativeConfigureDiagnosticColor(
    color === DiagnosticColor.Ansi
      ? NativeDiagnosticColor.Ansi
      : NativeDiagnosticColor.None,
  );
}
