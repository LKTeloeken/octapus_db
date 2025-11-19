import { cva, type VariantProps } from "class-variance-authority";

export const modalVariants = cva("", {
  variants: {
    maxWidth: {
      xs: "max-w-[400px]",
      sm: "max-w-[600px]",
      md: "max-w-[900px]",
      lg: "max-w-[1200px]",
      xl: "max-w-[1536px]",
      "2xl": "max-w-[1600px]",
      full: "max-w-full",
    },
  },
  defaultVariants: {
    maxWidth: "md",
  },
});

export interface GenericModalProps extends VariantProps<typeof modalVariants> {
  open: boolean;
  setOpen: (open: boolean) => void;
  Trigger?: React.ForwardRefExoticComponent<any>;
  closeButton?: React.ReactNode;
  title?: string;
  description?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  showCloseButton?: boolean;
  loading?: boolean;
}
