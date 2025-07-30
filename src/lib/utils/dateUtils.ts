export function formatDate(timestamp: number): string {
  if (!timestamp || typeof timestamp !== 'number' || isNaN(timestamp)) {
    return 'No date';
  }

  const date = new Date(timestamp);
  if (isNaN(date.getTime())) {
    return 'Invalid date';
  }

  // Changed to dd/mm/yy format
  return date.toLocaleDateString('en-GB', {
    day: '2-digit',
    month: '2-digit',
    year: '2-digit'
  });
}

export function formatRelativeTime(timestamp: number): string {
  if (!timestamp || typeof timestamp !== 'number' || isNaN(timestamp)) {
    return 'No date';
  }

  const date = new Date(timestamp);
  if (isNaN(date.getTime())) {
    return 'Invalid date';
  }

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffHours / 24);

  if (diffMs < 60000) return 'Just now';
  if (diffHours < 1) return `${Math.floor(diffMs / 60000)} minutes ago`;
  if (diffHours < 24) return `${diffHours} hours ago`;
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays} days ago`;

  // Changed to dd/mm/yy format for older dates
  return date.toLocaleDateString('en-GB', {
    day: '2-digit',
    month: '2-digit',
    year: '2-digit'
  });
}

export function isValidDate(timestamp: number): boolean {
  if (!timestamp || typeof timestamp !== 'number' || isNaN(timestamp)) {
    return false;
  }
  const date = new Date(timestamp);
  return !isNaN(date.getTime());
}

export function isToday(timestamp: number): boolean {
  if (!isValidDate(timestamp)) {
    return false;
  }

  let adjustedTimestamp = timestamp;
  if (timestamp < 1000000000000) {
    adjustedTimestamp = timestamp * 1000;
  }

  const date = new Date(adjustedTimestamp);
  const today = new Date();
  return date.toDateString() === today.toDateString();
}
