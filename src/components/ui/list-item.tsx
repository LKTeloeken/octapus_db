import React, { ReactNode, HTMLAttributes, ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

// ListItem: basic container with optional padding and secondary action
export interface ListItemProps extends HTMLAttributes<HTMLLIElement> {
  disablePadding?: boolean;
  secondaryAction?: ReactNode;
  children: ReactNode;
}
export const ListItem = React.forwardRef<HTMLLIElement, ListItemProps>(
  (
    { disablePadding = false, secondaryAction, className, children, ...props },
    ref
  ) => (
    <li
      ref={ref}
      className={cn(
        "relative flex items-center w-full",
        !disablePadding && "px-4 py-2",
        className
      )}
      {...props}
    >
      <div className="flex items-center space-x-4 w-full">{children}</div>

      {secondaryAction && (
        <div className="absolute right-4 top-1/2 transform -translate-y-1/2 flex-shrink-0">
          {secondaryAction}
        </div>
      )}
    </li>
  )
);
ListItem.displayName = "ListItem";

// ListItemButton: interactive element with selected state
export interface ListItemButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement> {
  selected?: boolean;
  children: ReactNode;
}
export const ListItemButton = React.forwardRef<
  HTMLButtonElement,
  ListItemButtonProps
>(({ selected = false, className, children, ...props }, ref) => (
  <button
    ref={ref}
    className={cn(
      "flex items-center w-full text-left space-x-4 px-4 py-2 rounded-md transition-colors cursor-pointer",
      selected
        ? "bg-gray-100 dark:bg-gray-800"
        : "hover:bg-gray-100 dark:hover:bg-gray-800",
      className
    )}
    {...props}
  >
    {children}
  </button>
));
ListItemButton.displayName = "ListItemButton";

// ListItemIcon: icon wrapper
export interface ListItemIconProps extends HTMLAttributes<HTMLSpanElement> {
  children: ReactNode;
}
export const ListItemIcon = React.forwardRef<
  HTMLSpanElement,
  ListItemIconProps
>(({ className, children, ...props }, ref) => (
  <span
    ref={ref}
    className={cn(
      "flex-shrink-0 w-6 h-6 text-gray-500 dark:text-gray-400",
      className
    )}
    {...props}
  >
    {children}
  </span>
));
ListItemIcon.displayName = "ListItemIcon";

// ListItemAvatar: avatar wrapper
export interface ListItemAvatarProps extends HTMLAttributes<HTMLDivElement> {
  src: string;
  alt?: string;
}
export const ListItemAvatar = React.forwardRef<
  HTMLDivElement,
  ListItemAvatarProps
>(({ className, src, alt = "avatar", ...props }, ref) => (
  <div ref={ref} className={cn("flex-shrink-0", className)} {...props}>
    <img src={src} alt={alt} className="w-8 h-8 rounded-full object-cover" />
  </div>
));
ListItemAvatar.displayName = "ListItemAvatar";

// ListItemText: primary and optional secondary text
export interface ListItemTextProps {
  primary: ReactNode;
  secondary?: ReactNode;
  className?: string;
}
export const ListItemText: React.FC<ListItemTextProps> = ({
  primary,
  secondary,
  className,
}) => (
  <div className={cn("flex flex-col", className)}>
    <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
      {primary}
    </span>
    {secondary && (
      <span className="text-xs text-gray-500 dark:text-gray-400">
        {secondary}
      </span>
    )}
  </div>
);
