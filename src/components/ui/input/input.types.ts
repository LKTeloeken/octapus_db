import type { ButtonProps } from '@/components/ui/button';
import type {
  ComponentProps,
  InputHTMLAttributes,
  MouseEventHandler,
  ReactNode,
} from 'react';

export type InputFieldType = 'text' | 'password' | 'number' | 'file';

export interface InputFieldActionButton extends ButtonProps {
  /** Text or content rendered inside the action button. */
  label: ReactNode;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  disabled?: boolean;
  className?: string;
}

export interface InputFieldProps extends Omit<
  ComponentProps<'input'>,
  'size' | 'type'
> {
  /** Label displayed above the input. Omit for placeholder-only fields. */
  label?: ReactNode;

  placeholder?: string;

  /** Helper text displayed below the input. Shown in destructive color when `error` is true. */
  helperText?: ReactNode;

  /** When true, applies error styles and sets `aria-invalid` on the input. */
  error?: boolean;

  /** When true, the field stretches to fill the available width. */
  fullWidth?: boolean;

  required?: boolean;

  type?: InputFieldType;

  /** Class applied to the root wrapper. */
  className?: string;

  /** Class applied to the label element. Equivalent to MUI `InputLabelProps.className`. */
  labelClassName?: string;

  /** Class applied to the native input element. */
  inputClassName?: string;

  /** Class applied to the helper text element. Equivalent to MUI `FormHelperTextProps.className`. */
  helperTextClassName?: string;

  /** Class applied to the bordered control wrapper around the input. */
  controlClassName?: string;

  /**
   * Material-style input adornments and class overrides.
   * `className` is merged with `inputClassName`.
   */
  InputProps?: {
    startAdornment?: ReactNode;
    endAdornment?: ReactNode;
    className?: string;
  };

  /**
   * Optional action button rendered to the right of the input,
   * each with its own border (as in the design mockup).
   */
  actionButton?: InputFieldActionButton;

  size?: 'sm' | 'default' | 'lg';

  /** Extra native attributes forwarded to the underlying input element. */
  inputProps?: InputHTMLAttributes<HTMLInputElement>;
}

export interface UseInputProps {
  id?: string;
  error?: boolean;
  helperText?: ReactNode;
  InputProps?: InputFieldProps['InputProps'];
}

export interface UseInputReturn {
  inputId: string;
  helperId: string | undefined;
  isInvalid: boolean;
  startAdornment: ReactNode | undefined;
  endAdornment: ReactNode | undefined;
  inputAdornmentClassName: string | undefined;
}
