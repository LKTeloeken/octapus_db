import { cva, type VariantProps } from 'class-variance-authority';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import type { InputFieldProps } from './input.types';
import { useInput } from './use-input';

/**
 * Campo do Ink: fundo rebaixado (`--field`), borda `--line` que reforça no
 * hover; no foco a borda vira Iris com halo de 3 px (`--iris-soft`); em erro,
 * o mesmo desenho em `--danger`.
 */
export const inputControlVariants = cva(
  [
    'flex min-w-0 items-center rounded-sm border border-line bg-field text-fg',
    'transition-[border-color,box-shadow] outline-none hover:border-line-strong',
    'focus-within:border-iris-text focus-within:ring-3 focus-within:ring-iris-soft focus-within:hover:border-iris-text',
    'has-disabled:pointer-events-none has-disabled:cursor-not-allowed has-disabled:opacity-50',
    "[&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 [&_svg]:shrink-0",
  ],
  {
    variants: {
      size: {
        sm: 'h-7 gap-1.5 px-2 text-body',
        default: 'h-8 gap-2 px-2.5 text-body',
        lg: 'h-9 gap-2 px-3 text-heading',
      },
      error: {
        true: 'border-danger hover:border-danger focus-within:border-danger focus-within:ring-danger-soft focus-within:hover:border-danger',
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
          className={cn(isInvalid && 'text-danger', labelClassName)}
        >
          {label}
          {required && (
            <span className="text-danger" aria-hidden>
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
              className="flex shrink-0 items-center text-fg-subtle"
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
              'file:text-fg placeholder:text-fg-subtle',
              'selection:bg-iris/40 selection:text-fg',
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
              className="flex shrink-0 items-center text-fg-subtle"
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
            'text-small',
            isInvalid ? 'text-danger' : 'text-fg-subtle',
            helperTextClassName,
          )}
        >
          {helperText}
        </p>
      )}
    </div>
  );
}
