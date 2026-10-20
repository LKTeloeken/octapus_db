import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { DB_TYPE_BG_COLOR } from '@/lib/db-defaults';
import { cn } from '@/lib/utils';
import type { useTargetPicker } from './use-target-picker';

type TargetPickerProps = ReturnType<typeof useTargetPicker>;

/** Servidor e database da conversa — a origem da estrutura */
export function TargetPicker({
  servers,
  loading,
  serverValue,
  selectServer,
  server,
  databases,
  databaseValue,
  selectDatabase,
  databasesLoading,
  disabled,
}: TargetPickerProps) {
  const noServers = !loading && servers.length === 0;

  return (
    <div className="flex min-w-0 items-center gap-1.5">
      <Select
        value={serverValue}
        onValueChange={selectServer}
        disabled={disabled || noServers}
      >
        <SelectTrigger
          size="sm"
          aria-label="Servidor"
          className="min-w-0 max-w-44 shrink"
        >
          <SelectValue
            placeholder={
              loading
                ? 'Carregando…'
                : noServers
                  ? 'Nenhum servidor compatível'
                  : 'Servidor'
            }
          />
        </SelectTrigger>
        <SelectContent>
          {servers.map(item => (
            <SelectItem key={item.id} value={String(item.id)}>
              <span
                aria-hidden="true"
                className={cn(
                  'size-1.5 shrink-0 rounded-full',
                  DB_TYPE_BG_COLOR[item.dbType],
                )}
              />
              <span className="truncate">{item.name}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={databaseValue}
        onValueChange={selectDatabase}
        disabled={disabled || !server}
      >
        <SelectTrigger
          size="sm"
          aria-label="Database"
          className="min-w-0 max-w-44 shrink font-mono text-small"
        >
          <SelectValue
            placeholder={databasesLoading ? 'Carregando…' : 'Database'}
          />
        </SelectTrigger>
        <SelectContent>
          {databases.map(name => (
            <SelectItem key={name} value={name} className="font-mono">
              {name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
