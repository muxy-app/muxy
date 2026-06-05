export {};

declare global {
  interface MuxyTheme {
    colorScheme: "light" | "dark";
    accent?: string;
  }

  interface MuxyExecResult {
    exitCode: number;
    stdout: string;
    stderr: string;
    timedOut?: boolean;
  }

  interface MuxyToastOptions {
    title?: string;
    body: string;
    variant?: "success" | "error" | "info" | "warning";
  }

  type MuxyIcon = string | { symbol: string } | { svg: string };
  type MuxyExtensionEventName = `extension.${string}`;

  interface MuxyBridge {
    extensionID: string;
    data: Record<string, unknown> | null;
    theme?: MuxyTheme;
    onThemeChange(handler: (theme: MuxyTheme) => void): () => void;
    onDataChange(handler: (data: Record<string, unknown> | null) => void): () => void;
    panels: {
      open(panelID: string, data?: Record<string, unknown>): Promise<void>;
      toggle(panelID: string, data?: Record<string, unknown>): Promise<void>;
      close(panelID: string): Promise<void>;
    };
    topbar: {
      set(opts: { id: string; icon?: MuxyIcon; visible?: boolean }): Promise<void>;
      show(id: string): Promise<void>;
      hide(id: string): Promise<void>;
    };
    exec(argv: string[], options?: { cwd?: string; timeoutMs?: number }): Promise<MuxyExecResult>;
    toast(opts: MuxyToastOptions): Promise<void>;
    events: {
      subscribe(name: string, handler: (payload: unknown) => void): () => void;
      emit(name: MuxyExtensionEventName, payload?: unknown): Promise<void>;
    };
  }

  interface Window {
    muxy: MuxyBridge;
  }
  const muxy: MuxyBridge;
}
