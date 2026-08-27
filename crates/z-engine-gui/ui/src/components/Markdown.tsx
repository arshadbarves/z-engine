import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

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
  return (
    <SyntaxHighlighter
      language={match?.[1] ?? "text"}
      style={oneDark}
      PreTag="div"
      customStyle={{
        margin: "10px 0",
        borderRadius: 8,
        border: "1px solid var(--border)",
        fontSize: 12.5,
        background: "#0d0d10",
      }}
    >
      {text}
    </SyntaxHighlighter>
  );
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
