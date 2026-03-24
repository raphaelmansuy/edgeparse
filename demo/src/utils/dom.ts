/** DOM helper utilities. */

const DOM_PROPERTIES = new Set([
  'innerHTML', 'textContent', 'className', 'value', 'checked',
  'disabled', 'type', 'href', 'src', 'style',
]);

/**
 * Convert a camelCase ARIA property name (e.g. `ariaLabel`) to its
 * hyphenated attribute name (e.g. `aria-label`).
 * Returns `null` if the key is not a recognized ARIA prop.
 */
function toAriaAttr(key: string): string | null {
  if (key === 'role') return 'role';
  if (!key.startsWith('aria') || key.length <= 4) return null;
  // ariaLabel → aria-label, ariaPressed → aria-pressed
  return 'aria-' + key.slice(4, 5).toLowerCase() + key.slice(5).replace(/[A-Z]/g, c => '-' + c.toLowerCase());
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs?: Record<string, string>,
  ...children: (string | Node)[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (DOM_PROPERTIES.has(k)) {
        (element as unknown as Record<string, unknown>)[k] = v;
      } else {
        const ariaAttr = toAriaAttr(k);
        element.setAttribute(ariaAttr ?? k, v);
      }
    }
  }
  for (const child of children) {
    if (typeof child === 'string') {
      element.appendChild(document.createTextNode(child));
    } else {
      element.appendChild(child);
    }
  }
  return element;
}

export function $(selector: string, parent: ParentNode = document): HTMLElement | null {
  return parent.querySelector(selector);
}
