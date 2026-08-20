import { cva, type VariantProps } from 'class-variance-authority';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import type { InputFieldProps } from './input.types';
import { useInput } from './use-input';

export const inputControlVariants = cva(
  [
    'flex min-w-0 items-center rounded-md border bg-transparent shadow-xs',
    'border-input dark:bg-input/30',
    'transition-[color,box-shadow] outline-none',
    'focus-within:border-ring focus-within:ring-ring/50 focus-within:ring-[3px]',
    'has-disabled:pointer-events-none has-disabled:cursor-not-allowed has-disabled:opacity-50',
    "[&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 [&_svg]:shrink-0",
  ],
  {
    variants: {
      size: {
        sm: 'h-8 gap-1.5 px-2 text-xs',
        default: 'h-9 gap-2 px-3 text-sm',
        lg: 'h-10 gap-2 px-4 text-base',
      },
      error: {
        true: 'border-destructive ring-destructive/20 dark:ring-destructive/40 focus-within:border-destructive focus-within:ring-destructive/20',
        false: '',
      },
    },
    defaultVariants: {
      size: 'default',
      error: false,
    },
  },
);

export type InputControlVariants = VariantProps<typeof inputControlVariants>;

const actionButtonSize = (
  size: NonNullable<InputFieldProps['size']>,
): 'sm' | 'default' | 'lg' => {
  if (size === 'sm') return 'sm';
  if (size === 'lg') return 'lg';
  return 'default';
};

export function Input({
  label,
  placeholder,
  helperText,
  error = false,
  fullWidth = true,
  required,
  type = 'text',
  className,
  labelClassName,
  inputClassName,
  helperTextClassName,
  controlClassName,
  InputProps,
  actionButton,
  size = 'default',
  inputProps,
  id,
  disabled,
  ref,
  ...rest
}: InputFieldProps) {
  const {
    inputId,
    helperId,
    isInvalid,
    startAdornment,
    endAdornment,
    inputAdornmentClassName,
  } = useInput({ id, error, helperText, InputProps });

  const {
    label: actionLabel,
    variant: actionVariant = 'outline',
    size: actionSize,
    ...actionRest
  } = actionButton ?? {};

  return (
    <div
      data-slot="input-root"
      className={cn(
        'flex flex-col gap-1.5',
        fullWidth ? 'w-full' : 'w-fit',
        className,
      )}
    >
      {label != null && (
        <Label
          htmlFor={inputId}
          data-slot="input-label"
          className={cn(isInvalid && 'text-destructive', labelClassName)}
        >
          {label}
          {required && (
            <span className="text-destructive" aria-hidden>
              *
            </span>
          )}
        </Label>
      )}

      <div
        data-slot="input-row"
        className={cn('flex items-center gap-2', fullWidth && 'w-full')}
      >
        <div
          data-slot="input-control"
          data-invalid={isInvalid || undefined}
          className={cn(
            inputControlVariants({ size, error: isInvalid }),
            fullWidth && (actionButton ? 'flex-1' : 'w-full'),
            controlClassName,
          )}
        >
          {startAdornment != null && (
            <span
              data-slot="input-start-adornment"
              className="flex shrink-0 items-center text-muted-foreground"
            >
              {startAdornment}
            </span>
          )}

          <input
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="off"
            {...rest}
            {...inputProps}
            id={inputId}
            ref={ref}
            type={type}
            placeholder={placeholder}
            required={required}
            disabled={disabled}
            aria-invalid={isInvalid || undefined}
            aria-describedby={helperId}
            data-slot="input"
            className={cn(
              'h-full min-w-0 flex-1 bg-transparent outline-none',
              'file:text-foreground placeholder:text-muted-foreground',
              'selection:bg-primary selection:text-primary-foreground',
              'file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium',
              'disabled:pointer-events-none disabled:cursor-not-allowed',
              inputClassName,
              inputAdornmentClassName,
              inputProps?.className,
            )}
          />

          {endAdornment != null && (
            <span
              data-slot="input-end-adornment"
              className="flex shrink-0 items-center text-muted-foreground"
            >
              {endAdornment}
            </span>
          )}
        </div>

        {actionButton && (
          <Button
            variant={actionVariant}
            size={actionSize ?? actionButtonSize(size)}
            {...actionRest}
            type="button"
          >
            {actionLabel}
          </Button>
        )}
      </div>

      {helperText != null && (
        <p
          id={helperId}
          data-slot="input-helper"
          className={cn(
            'text-xs',
            isInvalid ? 'text-destructive' : 'text-muted-foreground',
            helperTextClassName,
          )}
        >
          {helperText}
        </p>
      )}
    </div>
  );
}
