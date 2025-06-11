import React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const typographyVariants = cva("", {
  variants: {
    variant: {
      h1: "scroll-m-20 text-4xl font-extrabold tracking-tight text-balance",
      h2: "mt-10 scroll-m-20 border-b pb-2 text-3xl font-semibold tracking-tight first:mt-0",
      h3: "mt-8 scroll-m-20 text-2xl font-semibold tracking-tight",
      h4: "scroll-m-20 text-xl font-semibold tracking-tight",
      p: "leading-7 [&:not(:first-child)]:mt-6",
      blockquote: "mt-6 border-l-2 pl-6 italic",
      list: "my-6 ml-6 list-disc [&>li]:mt-2",
      tableWrapper: "my-6 w-full overflow-y-auto",
    },
  },
  defaultVariants: {
    variant: "p",
  },
});

// Mapeia variantes “não-semânticas” para tags válidas
const elementMap: Record<string, React.ElementType> = {
  list: "ul",
  tableWrapper: "div",
};

export interface TypographyProps
  extends React.HTMLAttributes<HTMLElement>,
    VariantProps<typeof typographyVariants> {
  /** Força outro elemento HTML (ex: `<span>`) */
  as?: React.ElementType;
}

export const Typography = React.forwardRef<HTMLElement, TypographyProps>(
  ({ variant = "p", as, className, children, ...props }, ref) => {
    const Component = as ?? elementMap[variant || ""] ?? variant;

    return (
      <Component
        ref={ref as any}
        className={cn(typographyVariants({ variant }), className)}
        {...props}
      >
        {children}
      </Component>
    );
  }
);
Typography.displayName = "Typography";
