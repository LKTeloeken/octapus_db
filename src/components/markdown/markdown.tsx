import ReactMarkdown, { type Components } from 'react-markdown';
import { cn } from '@/lib/utils';

export interface MarkdownProps {
  children: string;
  /** Links abrem fora do app (o navegador do sistema), nunca no WebView */
  onOpenLink?: (href: string) => void;
  className?: string;
}

/**
 * Markdown de texto vindo de fora (termo de consentimento). Sem HTML cru: o
 * `react-markdown` não interpreta tags, só a sintaxe do Markdown. Tipografia do
 * Ink (DESIGN.md §3) — nada passa de 16 px. Carregado sob demanda
 * (`React.lazy`), fora do bundle principal.
 */
export default function Markdown({
  children,
  onOpenLink,
  className,
}: MarkdownProps) {
  const components: Components = {
    h1: ({ children }) => (
      <h3 className="text-title font-semibold text-fg">{children}</h3>
    ),
    h2: ({ children }) => (
      <h4 className="text-heading font-semibold text-fg">{children}</h4>
    ),
    h3: ({ children }) => (
      <h5 className="text-body font-semibold text-fg">{children}</h5>
    ),
    p: ({ children }) => <p className="text-body text-fg-muted">{children}</p>,
    ul: ({ children }) => (
      <ul className="flex list-disc flex-col gap-1 pl-5 text-body text-fg-muted">
        {children}
      </ul>
    ),
    ol: ({ children }) => (
      <ol className="flex list-decimal flex-col gap-1 pl-5 text-body text-fg-muted">
        {children}
      </ol>
    ),
    strong: ({ children }) => (
      <strong className="font-semibold text-fg">{children}</strong>
    ),
    code: ({ children }) => (
      <code className="rounded-xs bg-hover px-1 font-mono text-small text-fg">
        {children}
      </code>
    ),
    a: ({ children, href }) => (
      <a
        href={href}
        className="text-iris-text underline underline-offset-2 outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring"
        onClick={event => {
          event.preventDefault();
          if (href) onOpenLink?.(href);
        }}
      >
        {children}
      </a>
    ),
    hr: () => <hr className="border-line-subtle" />,
    img: () => null,
  };

  return (
    <div className={cn('flex flex-col gap-3', className)}>
      <ReactMarkdown components={components} skipHtml>
        {children}
      </ReactMarkdown>
    </div>
  );
}
