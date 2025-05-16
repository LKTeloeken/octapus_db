import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const listItemVariants = cva("flex items-center w-full text-sm rounded-md", {
  variants: {
    variant: {
      default: "bg-background",
      destructive: "text-destructive",
    },
    size: {
      default: "h-10",
      sm: "h-8",
      lg: "h-12",
    },
    disablePadding: {
      true: "px-0 py-0",
      false: "px-4 py-2",
    },
  },
  defaultVariants: {
    variant: "default",
    size: "default",
    disablePadding: false,
  },
});

function ListItem({
  className,
  variant,
  size,
  disablePadding,
  secondaryAction,
  ...props
}: React.ComponentProps<"div"> &
  VariantProps<typeof listItemVariants> & {
    secondaryAction?: React.ReactNode;
  }) {
  return (
    <div className="relative">
      {secondaryAction && (
        <div className="absolute right-0 top-1/2 transform -translate-y-1/2">
          {secondaryAction}
        </div>
      )}
      <div
        data-slot="list-item"
        className={cn(
          listItemVariants({ variant, size, disablePadding }),
          className
        )}
        {...props}
      >
        {props.children}
      </div>
    </div>
  );
}

const listItemTextVariants = cva("flex flex-col flex-grow", {
  variants: {
    variant: {
      default: "text-foreground",
      secondary: "text-muted-foreground",
    },
  },
  defaultVariants: {
    variant: "default",
  },
});

interface ListItemTextProps
  extends React.ComponentProps<"div">,
    VariantProps<typeof listItemTextVariants> {
  primary?: React.ReactNode;
  secondary?: React.ReactNode;
}

function ListItemText({
  className,
  variant,
  primary,
  secondary,
  ...props
}: ListItemTextProps) {
  return (
    <div
      data-slot="list-item-text"
      className={cn(listItemTextVariants({ variant }), className)}
      {...props}
    >
      {primary && (
        <span className="text-sm font-medium leading-none">{primary}</span>
      )}
      {secondary && (
        <span className="text-xs text-muted-foreground mt-1">{secondary}</span>
      )}
      {props.children}
    </div>
  );
}

const listItemButtonVariants = cva(
  "cursor-pointer flex items-center w-full px-4 py-2 text-sm rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none",
  {
    variants: {
      variant: {
        default: "hover:bg-accent hover:text-accent-foreground",
        destructive:
          "hover:bg-destructive/10 text-destructive hover:text-destructive",
      },
      size: {
        default: "h-10",
        sm: "h-8",
        lg: "h-12",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);

interface ListItemButtonProps
  extends React.ComponentProps<"button">,
    VariantProps<typeof listItemButtonVariants> {
  asChild?: boolean;
}

function ListItemButton({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ListItemButtonProps) {
  const Comp = asChild ? Slot : "button";

  return (
    <Comp
      data-slot="list-item-button"
      className={cn(listItemButtonVariants({ variant, size }), className)}
      {...props}
    />
  );
}

export {
  ListItem,
  ListItemText,
  ListItemButton,
  listItemVariants,
  listItemButtonVariants,
  listItemTextVariants,
};
