import { HugeiconsIcon } from '@hugeicons/react';
import { ArrowDown01Icon } from '@hugeicons/core-free-icons';
import { memo } from 'react';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  VALUE_ENCODINGS,
  type ValueEncoding,
  type ValueFormat,
} from '@/lib/value-format';

export const VALUE_FORMAT_LABELS: Record<ValueFormat, string> = {
  binary: 'Binário',
  html: 'HTML',
  json: 'JSON',
  text: 'Texto',
  xml: 'XML',
};

const FORMAT_ORDER: ValueFormat[] = ['binary', 'html', 'json', 'text', 'xml'];

interface ValueFormatMenuProps {
  format: ValueFormat;
  wordWrap: boolean;
  autoFormat: boolean;
  saveCompact: boolean;
  encoding: ValueEncoding;
  onFormatChange: (format: ValueFormat) => void;
  onWordWrapChange: (wordWrap: boolean) => void;
  onAutoFormatChange: (autoFormat: boolean) => void;
  onSaveCompactChange: (saveCompact: boolean) => void;
  onEncodingChange: (encoding: ValueEncoding) => void;
}

/** Visualização (Binário/HTML/JSON/Texto/XML) e as opções de formatação. */
export const ValueFormatMenu = memo(
  ({
    format,
    wordWrap,
    autoFormat,
    saveCompact,
    encoding,
    onFormatChange,
    onWordWrapChange,
    onAutoFormatChange,
    onSaveCompactChange,
    onEncodingChange,
  }: ValueFormatMenuProps) => {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="inline-flex h-6 items-center gap-1 rounded border border-border px-1.5 text-[11px] font-medium hover:bg-muted/60 data-[state=open]:bg-muted transition-colors outline-none"
            title="Formato de exibição"
          >
            {VALUE_FORMAT_LABELS[format]}
            <HugeiconsIcon icon={ArrowDown01Icon} className="size-3" />
          </button>
        </DropdownMenuTrigger>

        <DropdownMenuContent align="end" className="w-60">
          <DropdownMenuRadioGroup
            value={format}
            onValueChange={next => onFormatChange(next as ValueFormat)}
          >
            {FORMAT_ORDER.map(option => (
              <DropdownMenuRadioItem key={option} value={option}>
                {VALUE_FORMAT_LABELS[option]}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>

          <DropdownMenuSeparator />

          <DropdownMenuCheckboxItem
            checked={wordWrap}
            onCheckedChange={checked => onWordWrapChange(checked === true)}
            onSelect={event => event.preventDefault()}
          >
            Quebra de linha
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={autoFormat}
            onCheckedChange={checked => onAutoFormatChange(checked === true)}
            onSelect={event => event.preventDefault()}
          >
            Formatar automaticamente
          </DropdownMenuCheckboxItem>
          <DropdownMenuCheckboxItem
            checked={saveCompact}
            onCheckedChange={checked => onSaveCompactChange(checked === true)}
            onSelect={event => event.preventDefault()}
          >
            Salvar compactado (minificado)
          </DropdownMenuCheckboxItem>

          <DropdownMenuSeparator />

          <DropdownMenuSub>
            <DropdownMenuSubTrigger>
              Codificação
              <span className="ml-auto text-[10px] text-muted-foreground">
                {VALUE_ENCODINGS.find(e => e.value === encoding)?.label}
              </span>
            </DropdownMenuSubTrigger>
            <DropdownMenuSubContent className="w-52">
              <DropdownMenuRadioGroup
                value={encoding}
                onValueChange={next => onEncodingChange(next as ValueEncoding)}
              >
                {VALUE_ENCODINGS.map(option => (
                  <DropdownMenuRadioItem
                    key={option.value}
                    value={option.value}
                  >
                    {option.label}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  },
);

ValueFormatMenu.displayName = 'ValueFormatMenu';
