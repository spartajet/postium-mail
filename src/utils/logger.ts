import { warn, debug, trace, info, error } from "@tauri-apps/plugin-log";

export enum LogLevel {
  Trace = "trace",
  Debug = "debug",
  Info = "info",
  Warn = "warn",
  Error = "error",
}

class Logger {
  private context: string;

  constructor(context: string) {
    this.context = context;
  }

  private formatMessage(message: string, ...args: any[]): string {
    const timestamp = new Date().toISOString();
    const formattedArgs = args.length > 0 ? JSON.stringify(args) : "";
    return `[${timestamp}] [${this.context}] ${message} ${formattedArgs}`.trim();
  }

  async trace(message: string, ...args: any[]) {
    const formatted = this.formatMessage(message, ...args);
    await trace(formatted);
    if (import.meta.env.DEV) {
      console.trace(`[TRACE] ${formatted}`);
    }
  }

  async debug(message: string, ...args: any[]) {
    const formatted = this.formatMessage(message, ...args);
    await debug(formatted);
    if (import.meta.env.DEV) {
      console.debug(`[DEBUG] ${formatted}`);
    }
  }

  async info(message: string, ...args: any[]) {
    const formatted = this.formatMessage(message, ...args);
    await info(formatted);
    if (import.meta.env.DEV) {
      console.info(`[INFO] ${formatted}`);
    }
  }

  async warn(message: string, ...args: any[]) {
    const formatted = this.formatMessage(message, ...args);
    await warn(formatted);
    if (import.meta.env.DEV) {
      console.warn(`[WARN] ${formatted}`);
    }
  }

  async error(message: string, errorObj?: Error | unknown, ...args: any[]) {
    let errorDetails = "";
    if (errorObj instanceof Error) {
      errorDetails = `\nError: ${errorObj.message}\nStack: ${errorObj.stack}`;
    } else if (errorObj) {
      errorDetails = `\nError Details: ${JSON.stringify(errorObj)}`;
    }

    const formatted = this.formatMessage(message + errorDetails, ...args);
    await error(formatted);
    if (import.meta.env.DEV) {
      console.error(`[ERROR] ${formatted}`);
    }
  }

  async logPerformance(operation: string, fn: () => Promise<any>) {
    const startTime = performance.now();
    try {
      const result = await fn();
      const duration = performance.now() - startTime;
      await this.debug(`${operation} completed in ${duration.toFixed(2)}ms`);
      return result;
    } catch (err) {
      const duration = performance.now() - startTime;
      await this.error(
        `${operation} failed after ${duration.toFixed(2)}ms`,
        err,
      );
      throw err;
    }
  }
}

// Create logger instances for different modules
export const createLogger = (context: string) => new Logger(context);

// Pre-created loggers for common modules
export const appLogger = createLogger("App");
export const emailLogger = createLogger("Email");
export const accountLogger = createLogger("Account");
export const uiLogger = createLogger("UI");
export const storeLogger = createLogger("Store");

// Global error handler
export const setupGlobalErrorHandler = () => {
  window.addEventListener("unhandledrejection", async (event) => {
    await appLogger.error("Unhandled Promise Rejection", event.reason);
  });

  window.addEventListener("error", async (event) => {
    await appLogger.error("Global Error", {
      message: event.message,
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno,
      error: event.error,
    });
  });
};
