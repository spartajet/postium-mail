import { getContext, setContext } from 'svelte';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
  id: number;
  type: ToastType;
  message: string;
  duration: number;
}

class ToastState {
  toasts = $state<Toast[]>([]);
  private nextId = 0;

  show(message: string, type: ToastType = 'info', duration = 3000) {
    const id = this.nextId++;
    this.toasts.push({ id, type, message, duration });
    return id;
  }

  success(message: string) {
    return this.show(message, 'success');
  }

  error(message: string) {
    return this.show(message, 'error', 5000);
  }

  info(message: string) {
    return this.show(message, 'info');
  }

  warning(message: string) {
    return this.show(message, 'warning', 4000);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter(t => t.id !== id);
  }
}

const TOAST_KEY = Symbol('toast');

export function createToastState() {
  const state = new ToastState();
  setContext(TOAST_KEY, state);
  return state;
}

export function getToastState() {
  return getContext<ToastState>(TOAST_KEY);
}
