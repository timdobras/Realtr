import Notification from '$lib/components/Notification.svelte';
import { stack } from '@svelte-put/async-stack';

// Create the notification stack with different variants
export const notificationStack = stack({ timeout: 3000 })
  .addVariant('success', Notification)
  .addVariant('error', { component: Notification, timeout: 5000 })
  .addVariant('info', Notification)
  .addVariant('warning', { component: Notification, timeout: 4000 })
  .build();

// Convenience functions for pushing notifications
export function showSuccess(message: string) {
  return notificationStack.push('success', {
    props: { message, type: 'success' }
  });
}

export function showError(message: string) {
  return notificationStack.push('error', {
    props: { message, type: 'error' }
  });
}

export function showInfo(message: string) {
  return notificationStack.push('info', {
    props: { message, type: 'info' }
  });
}

export function showWarning(message: string) {
  return notificationStack.push('warning', {
    props: { message, type: 'warning' }
  });
}
