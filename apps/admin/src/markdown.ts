import MarkdownIt from "markdown-it";
import { defineComponent, h, type VNodeChild } from "vue";

const markdown = new MarkdownIt({ html: false, linkify: false });
const allowedTags = new Set([
  "p",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "blockquote",
  "ul",
  "ol",
  "li",
  "strong",
  "em",
  "s",
  "a",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
]);

function safeHref(value: string | number | null): string | null {
  if (typeof value !== "string" || !value) return null;
  try {
    const parsed = new URL(value, "https://mindfolio.invalid");
    return ["http:", "https:", "mailto:"].includes(parsed.protocol)
      ? value
      : null;
  } catch {
    return null;
  }
}

markdown.validateLink = (url: string) => safeHref(url) !== null;

type Token = ReturnType<typeof markdown.parse>[number];
type Frame = {
  tag: string | null;
  props: Record<string, string | number>;
  children: VNodeChild[];
};

function renderTokens(tokens: Token[]): VNodeChild[] {
  const stack: Frame[] = [{ tag: null, props: {}, children: [] }];
  const append = (node: VNodeChild) =>
    stack[stack.length - 1].children.push(node);

  for (const token of tokens) {
    if (token.type === "inline") {
      renderTokens(token.children ?? []).forEach(append);
    } else if (token.nesting === 1) {
      const props: Frame["props"] = {};
      let tag: string | null = allowedTags.has(token.tag) ? token.tag : null;
      if (tag === "a") {
        const href = safeHref(token.attrGet("href"));
        if (href) {
          Object.assign(props, {
            href,
            target: "_blank",
            rel: "noopener noreferrer",
          });
        } else {
          tag = null;
        }
      }
      if (tag === "ol") {
        const start = Number(token.attrGet("start"));
        if (Number.isSafeInteger(start) && start > 1) props.start = start;
      }
      stack.push({ tag, props, children: [] });
    } else if (token.nesting === -1) {
      const frame = stack.pop();
      if (frame)
        append(
          frame.tag
            ? h(frame.tag, frame.props, frame.children)
            : frame.children,
        );
    } else if (token.type === "fence" || token.type === "code_block") {
      append(h("pre", h("code", token.content)));
    } else if (token.type === "code_inline") {
      append(h("code", token.content));
    } else if (token.type === "image") {
      append(
        h("span", { class: "markdown-image-alt" }, `[图片：${token.content}]`),
      );
    } else if (token.type === "hardbreak") {
      append(h("br"));
    } else if (token.type === "softbreak") {
      append(" ");
    } else if (token.type === "hr") {
      append(h("hr"));
    } else if (token.content) {
      append(token.content);
    }
  }
  return stack[0].children;
}

export const MarkdownPreview = defineComponent({
  name: "MarkdownPreview",
  props: {
    source: { type: String, required: true },
    label: { type: String, required: true },
  },
  setup(props) {
    return () =>
      h(
        "div",
        {
          class: "markdown-preview",
          role: "region",
          "aria-label": props.label,
        },
        props.source
          ? renderTokens(markdown.parse(props.source, {}))
          : "暂无说明",
      );
  },
});
