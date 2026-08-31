/** Keep an overlay mounted until its exit animation finishes. */

export function presence(open: () => boolean, exitMs = 280) {
  let mounted = $state(open());
  let closing = $state(false);

  $effect(() => {
    const isOpen = open();
    if (isOpen) {
      mounted = true;
      closing = false;
      return;
    }
    if (!mounted) return;
    closing = true;
    const id = window.setTimeout(() => {
      mounted = false;
      closing = false;
    }, exitMs);
    return () => window.clearTimeout(id);
  });

  return {
    get mounted() {
      return mounted;
    },
    get closing() {
      return closing;
    },
  };
}
