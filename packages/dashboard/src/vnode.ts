// A tiny element tree for pages whose text comes from outside (the briefs
// page shows brief text a human or an agent wrote). Text only ever becomes a
// DOM text node (`toDom`), so markup in a brief is shown, never run, and no
// page code assigns HTML. `toHtml` serializes the same tree, escaped, for
// tests and for reading what a page would show.

export type Child = VNode | string;

export interface VNode {
  tag: string;
  attrs: Record<string, string | boolean>;
  on: Record<string, (event: Event) => void>;
  children: Child[];
}

type Loose = Child | Child[] | null | false | undefined;

export function h(tag: string, attrs: Record<string, string | boolean> = {}, ...children: Loose[]): VNode {
  const flat: Child[] = [];
  for (const child of children) {
    if (child === null || child === false || child === undefined) continue;
    if (Array.isArray(child)) flat.push(...child);
    else flat.push(child);
  }
  return { tag, attrs, on: {}, children: flat };
}

export function withOn(node: VNode, on: VNode["on"]): VNode {
  node.on = { ...node.on, ...on };
  return node;
}

export function toDom(node: Child): Node {
  if (typeof node === "string") return document.createTextNode(node);
  const el = document.createElement(node.tag);
  for (const [name, value] of Object.entries(node.attrs)) {
    if (value === false) continue;
    el.setAttribute(name, value === true ? "" : value);
  }
  for (const [type, handler] of Object.entries(node.on)) el.addEventListener(type, handler);
  for (const child of node.children) el.append(toDom(child));
  return el;
}

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

export function toHtml(node: Child): string {
  if (typeof node === "string") return escapeHtml(node);
  const attrs = Object.entries(node.attrs)
    .flatMap(([name, value]) => (value === false ? [] : value === true ? [` ${name}`] : [` ${name}="${escapeHtml(value)}"`]))
    .join("");
  return `<${node.tag}${attrs}>${node.children.map(toHtml).join("")}</${node.tag}>`;
}

/** Every element in the tree, depth first. */
export function elements(node: Child): VNode[] {
  if (typeof node === "string") return [];
  return [node, ...node.children.flatMap(elements)];
}
