type Child = Node | string | null | undefined | false;

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  children: Child[] = [],
  attributes: Record<string, string> = {},
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  for (const [key, value] of Object.entries(attributes)) {
    node.setAttribute(key, value);
  }
  node.append(...children.filter(Boolean).map((child) => child instanceof Node ? child : document.createTextNode(String(child))));
  return node;
}

export function requireElement<T extends Element>(selector: string, type: { new (): T }): T {
  const element = document.querySelector(selector);
  if (!(element instanceof type)) {
    throw new Error(`Missing required UI element: ${selector}`);
  }
  return element;
}
