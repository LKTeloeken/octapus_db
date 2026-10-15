import { Alert02Icon, DatabaseIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { ImportCandidate, ImportPreview } from '@/api/types/import.types';
import type { ServerGroup } from '@/api/types/server.types';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { NO_GROUP } from '@/features/server-groups/server-groups.types';
import { DB_TYPE_ICONS } from '@/lib/db-defaults';
import { cn } from '@/lib/utils';
import { SOURCE_LABELS, candidateTarget, warningLabel } from './import-labels';

interface ReviewStepProps {
  result: ImportPreview;
  candidates: ImportCandidate[];
  selected: Set<number>;
  allChosen: boolean;
  someChosen: boolean;
  toggle: (key: number) => void;
  toggleAll: () => void;
  serverNames: Map<number, string>;
  sshCount: number;
  hasPasswords: boolean;
  keychainCount: number;
  includePasswords: boolean;
  setIncludePasswords: (value: boolean) => void;
  keepGroups: boolean;
  setKeepGroups: (value: boolean) => void;
  groups: ServerGroup[];
  targetGroupId: number | null;
  setTargetGroupId: (value: number | null) => void;
}

/** Passo 2: o que entra, com os avisos de cada conexão e as opções */
export function ReviewStep({
  result,
  candidates,
  selected,
  allChosen,
  someChosen,
  toggle,
  toggleAll,
  serverNames,
  sshCount,
  hasPasswords,
  keychainCount,
  includePasswords,
  setIncludePasswords,
  keepGroups,
  setKeepGroups,
  groups,
  targetGroupId,
  setTargetGroupId,
}: ReviewStepProps) {
  const folderCount = result.groups.length;

  return (
    <div className="flex min-h-0 flex-col gap-4">
      <div className="flex items-center gap-2.5 px-2.5">
        <Checkbox
          id="import-all"
          checked={allChosen ? true : someChosen ? 'indeterminate' : false}
          onCheckedChange={toggleAll}
          aria-label="Marcar todas"
        />
        <Label htmlFor="import-all" className="flex-1 text-small text-fg-muted">
          {candidates.length === 1
            ? `1 conexão de ${SOURCE_LABELS[result.source]}`
            : `${candidates.length} conexões de ${SOURCE_LABELS[result.source]}`}
        </Label>
        {result.path && (
          <span
            className="max-w-[45%] truncate font-mono text-[11px] text-fg-subtle"
            title={result.path}
          >
            {result.path}
          </span>
        )}
      </div>

      <ul className="-mx-1 max-h-[300px] overflow-y-auto scrollbar-thin px-1">
        {candidates.map(candidate => {
          const supported = candidate.dbType !== null;
          const isChecked = selected.has(candidate.key);
          const duplicate =
            candidate.duplicateOf !== null
              ? serverNames.get(candidate.duplicateOf)
              : undefined;
          const id = `import-${candidate.key}`;

          return (
            <li
              key={candidate.key}
              className={cn(
                'flex items-start gap-2.5 rounded-md px-2.5 py-2 transition-colors',
                supported && 'hover:bg-hover',
              )}
            >
              <Checkbox
                id={id}
                className="mt-0.5"
                checked={isChecked}
                disabled={!supported}
                onCheckedChange={() => toggle(candidate.key)}
              />
              {candidate.dbType ? (
                <img
                  src={DB_TYPE_ICONS[candidate.dbType]}
                  alt=""
                  className="mt-0.5 size-4 shrink-0"
                />
              ) : (
                <HugeiconsIcon
                  icon={DatabaseIcon}
                  className="mt-0.5 size-4 shrink-0 text-fg-subtle"
                />
              )}
              <label
                htmlFor={id}
                className={cn(
                  'flex min-w-0 flex-1 flex-col gap-0.5',
                  supported ? 'cursor-pointer' : 'cursor-default',
                )}
              >
                <span className="flex min-w-0 items-center gap-1.5">
                  <span
                    className={cn(
                      'truncate text-body',
                      supported ? 'text-fg' : 'text-fg-muted',
                    )}
                  >
                    {candidate.name}
                  </span>
                  {candidate.group && (
                    <span className="truncate text-small text-fg-subtle">
                      {candidate.group}
                    </span>
                  )}
                </span>
                <span className="truncate font-mono text-[11px] text-fg-subtle">
                  {candidateTarget(candidate)}
                </span>
              </label>
              <span className="mt-0.5 flex shrink-0 flex-wrap justify-end gap-1">
                {duplicate && (
                  <Badge
                    variant="info"
                    title={`Parece ser o servidor "${duplicate}"`}
                  >
                    Já existe
                  </Badge>
                )}
                {candidate.passwordInKeychain && (
                  <Badge title="A senha está no Keychain: o macOS pede permissão ao importar">
                    Keychain
                  </Badge>
                )}
                {candidate.warnings.map(warning => {
                  const { label, title, tone } = warningLabel(warning);
                  return (
                    <Badge
                      key={`${warning.kind}-${label}`}
                      variant={tone}
                      title={title}
                    >
                      {label}
                    </Badge>
                  );
                })}
              </span>
            </li>
          );
        })}
      </ul>

      {sshCount > 0 && (
        <div className="flex gap-2 rounded-md bg-warning-soft px-3 py-2.5 text-small text-fg">
          <HugeiconsIcon
            icon={Alert02Icon}
            className="mt-0.5 size-4 shrink-0 text-warning"
          />
          <p>
            {sshCount === 1 ? '1 conexão passa' : `${sshCount} conexões passam`}{' '}
            por túnel SSH, que o Octapus ainda não suporta. Elas são importadas,
            mas só conectam com o túnel aberto por fora (por exemplo,{' '}
            <span className="font-mono">ssh -L</span>) e o host trocado para{' '}
            <span className="font-mono">localhost</span>.
          </p>
        </div>
      )}

      <div className="flex flex-col gap-3 border-t border-line-subtle pt-4">
        {hasPasswords && (
          <OptionRow
            id="import-passwords"
            label="Importar senhas"
            help={
              keychainCount > 0
                ? `O macOS vai pedir permissão para ler ${keychainCount === 1 ? 'a senha' : `as ${keychainCount} senhas`} do Keychain.`
                : 'As senhas vão para o cofre cifrado do Octapus.'
            }
            checked={includePasswords}
            onChange={setIncludePasswords}
          />
        )}
        {folderCount > 0 && (
          <OptionRow
            id="import-groups"
            label="Recriar as pastas como grupos"
            help={
              folderCount === 1
                ? `A pasta "${result.groups[0]}" vira grupo (ou entra no que já tiver o nome).`
                : `${folderCount} pastas viram grupos (ou entram nos que já tiverem o nome).`
            }
            checked={keepGroups}
            onChange={setKeepGroups}
          />
        )}
        {groups.length > 0 && (
          <div className="flex items-center justify-between gap-3">
            <Label htmlFor="import-target-group" className="text-body text-fg">
              {folderCount > 0 && keepGroups ? 'Grupo das sem pasta' : 'Grupo'}
            </Label>
            <Select
              value={targetGroupId == null ? NO_GROUP : String(targetGroupId)}
              onValueChange={value =>
                setTargetGroupId(value === NO_GROUP ? null : Number(value))
              }
            >
              <SelectTrigger
                id="import-target-group"
                size="sm"
                className="w-48"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_GROUP}>Sem grupo</SelectItem>
                <SelectSeparator />
                {groups.map(group => (
                  <SelectItem key={group.id} value={String(group.id)}>
                    {group.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}
      </div>
    </div>
  );
}

function OptionRow({
  id,
  label,
  help,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  help: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-3">
      <div className="flex flex-col gap-0.5">
        <Label htmlFor={id} className="text-body text-fg">
          {label}
        </Label>
        <span className="text-small text-fg-subtle">{help}</span>
      </div>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}
