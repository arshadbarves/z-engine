import { useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { Check, Copy } from "lucide-react";

function CodeBlock({
  language,
  text,
}: {
  language: string;
  text: string;
}) {
  const [copied, setCopied] = useState(false);
  const lines = text.split("\n").length;

  async function onCopy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.error("Failed to copy code", e);
    }
  }

  return (
    <div className="code-block">
      <div className="code-block-head">
        <div className="code-meta-left">
          <span className="code-lang">{language || "text"}</span>
          <span className="code-lines">{lines} {lines === 1 ? "line" : "lines"}</span>
        </div>
        <button
          type="button"
          className="code-copy-btn"
          onClick={() => void onCopy()}
          title="Copy code to clipboard"
        >
          {copied ? (
            <>
              <Check size={11} className="copy-ok" />
              <span>Copied</span>
            </>
          ) : (
            <>
              <Copy size={11} />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <SyntaxHighlighter
        language={language || "text"}
        style={oneDark}
        PreTag="div"
        codeTagProps={{
          style: {
            background: "transparent",
            border: "none",
            boxShadow: "none",
            padding: 0,
            fontFamily: "inherit",
          },
        }}
        customStyle={{
          margin: 0,
          padding: "12px 16px",
          fontSize: 12.5,
          background: "#0c0d10",
          borderRadius: "0 0 var(--radius-s) var(--radius-s)",
          overflowX: "auto",
        }}
      >
        {text}
      </SyntaxHighlighter>
    </div>
  );
}

function Code({
  className,
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  const match = /language-(\w+)/.exec(className ?? "");
  const text = String(children ?? "").replace(/\n$/, "");
  if (!match && !text.includes("\n")) {
    return <code className={className}>{children}</code>;
  }
  return <CodeBlock language={match?.[1] ?? "text"} text={text} />;
}

/** GFM markdown renderer for assistant messages. */
export function AssistantMarkdown({ text }: { text: string }) {
  return (
    <div className="md">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{ code: Code, pre: ({ children }) => <>{children}</> }}
      >
        {text}
      </Markdown>
    </div>
  );
}
