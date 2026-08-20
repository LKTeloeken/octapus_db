import { useId } from 'react';
import type { UseInputProps, UseInputReturn } from './input.types';

export const useInput = ({
  id,
  error = false,
  helperText,
  InputProps,
}: UseInputProps): UseInputReturn => {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const helperId = helperText ? `${inputId}-helper` : undefined;

  return {
    inputId,
    helperId,
    isInvalid: error,
    startAdornment: InputProps?.startAdornment,
    endAdornment: InputProps?.endAdornment,
    inputAdornmentClassName: InputProps?.className,
  };
};
