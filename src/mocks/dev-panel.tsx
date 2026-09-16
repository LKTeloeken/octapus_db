import type { ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';
import { resetDataset } from './data';
import {
  DEFAULT_ERROR_MESSAGE,
  useMockStore,
  type FailMode,
} from './mock-store';

const FAIL_MODES: { value: FailMode; label: string }[] = [
  { value: 'off', label: 'nunca' },
  { value: 'next', label: 'próximo' },
  { value: 'always', label: 'sempre' },
];

/**
 * Painel de controle do backend simulado. Só existe em modo mock — montado por
 * `installMocks()` num root separado do `#root`.
 */
export const MockDevPanel = () => {
  const panelOpen = useMockStore(state => state.panelOpen);
  const setPanelOpen = useMockStore(state => state.setPanelOpen);

  if (!panelOpen) {
    return (
      <button
        type="button"
        onClick={() => setPanelOpen(true)}
        className="fixed bottom-3 right-3 z-[9999] rounded-md border border-border bg-background/90 px-2 py-1 font-mono text-[10px] font-bold tracking-widest text-muted-foreground shadow-md backdrop-blur transition-colors hover:text-foreground"
      >
        MOCK
      </button>
    );
  }

  return <ExpandedPanel onClose={() => setPanelOpen(false)} />;
};

const ExpandedPanel = ({ onClose }: { onClose: () => void }) => {
  const latencyMs = useMockStore(state => state.latencyMs);
  const failMode = useMockStore(state => state.failMode);
  const errorMessage = useMockStore(state => state.errorMessage);
  const emptyMode = useMockStore(state => state.emptyMode);
  const updateAvailable = useMockStore(state => state.updateAvailable);

  const setLatency = useMockStore(state => state.setLatency);
  const setFailMode = useMockStore(state => state.setFailMode);
  const setErrorMessage = useMockStore(state => state.setErrorMessage);
  const setEmptyMode = useMockStore(state => state.setEmptyMode);
  const setUpdateAvailable = useMockStore(state => state.setUpdateAvailable);

  /** Os dados vivem em memória; recarregar é o jeito mais honesto de zerar. */
  const resetData = () => {
    resetDataset();
    window.location.reload();
  };

  return (
    <div className="fixed bottom-3 right-3 z-[9999] w-72 rounded-lg border border-border bg-background/95 p-3 text-xs shadow-xl backdrop-blur">
      <header className="mb-3 flex items-center justify-between">
        <span className="font-mono text-[10px] font-bold tracking-widest text-muted-foreground">
          MOCK
        </span>
        <button
          type="button"
          onClick={onClose}
          className="text-muted-foreground transition-colors hover:text-foreground"
          aria-label="Fechar painel do mock"
        >
          ×
        </button>
      </header>

      <Field label="Latência" value={`${latencyMs} ms`}>
        <input
          type="range"
          min={0}
          max={2000}
          step={50}
          value={latencyMs}
          onChange={event => setLatency(Number(event.target.value))}
          className="w-full accent-primary"
        />
      </Field>

      <Field label="Falhar comando">
        <div className="flex gap-1">
          {FAIL_MODES.map(mode => (
            <button
              key={mode.value}
              type="button"
              onClick={() => setFailMode(mode.value)}
              className={cn(
                'flex-1 rounded border border-border px-1.5 py-1 transition-colors',
                failMode === mode.value
                  ? 'bg-primary text-primary-foreground border-primary'
                  : 'hover:bg-accent',
              )}
            >
              {mode.label}
            </button>
          ))}
        </div>
        {failMode !== 'off' && (
          <input
            value={errorMessage}
            onChange={event => setErrorMessage(event.target.value)}
            onBlur={() => {
              if (!errorMessage.trim()) setErrorMessage(DEFAULT_ERROR_MESSAGE);
            }}
            className="mt-1.5 w-full rounded border border-border bg-transparent px-1.5 py-1 font-mono text-[10px] outline-none focus:border-ring"
          />
        )}
      </Field>

      <Toggle
        label="Respostas vazias"
        hint="listagens devolvem []"
        checked={emptyMode}
        onChange={setEmptyMode}
      />

      <Toggle
        label="Update disponível"
        hint="liga o UpdateNotifier"
        checked={updateAvailable}
        onChange={setUpdateAvailable}
      />

      <Button
        variant="outline"
        size="sm"
        className="mt-3 h-7 w-full text-xs"
        onClick={resetData}
      >
        Resetar dados
      </Button>
    </div>
  );
};

const Field = ({
  label,
  value,
  children,
}: {
  label: string;
  value?: string;
  children: ReactNode;
}) => (
  <div className="mb-3">
    <div className="mb-1 flex items-center justify-between text-muted-foreground">
      <span>{label}</span>
      {value && <span className="font-mono">{value}</span>}
    </div>
    {children}
  </div>
);

const Toggle = ({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) => (
  <label className="mb-2 flex cursor-pointer items-center justify-between gap-2">
    <span className="flex flex-col">
      <span>{label}</span>
      <span className="text-[10px] text-muted-foreground">{hint}</span>
    </span>
    <Switch checked={checked} onCheckedChange={onChange} />
  </label>
);
