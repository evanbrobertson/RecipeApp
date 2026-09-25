/**
 * Grows a textarea with its content. Browsers with `field-sizing: content` do this in
 * CSS already, so the action only steps in where that's missing (e.g. older Firefox).
 */
export function autosize(node: HTMLTextAreaElement) {
  if (CSS.supports("field-sizing", "content")) return
  const resize = () => {
    node.style.height = "auto"
    node.style.height = `${node.scrollHeight + 2}px`
  }
  node.style.overflowY = "auto"
  node.addEventListener("input", resize)
  queueMicrotask(resize)
  return { destroy: () => node.removeEventListener("input", resize) }
}
