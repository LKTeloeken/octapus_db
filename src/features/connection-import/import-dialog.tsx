import { memo } from 'react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { useImportDialogStore } from '@/stores/import-dialog-store';
import { ReviewStep } from './review-step';
import { SourceStep } from './source-step';
import { useImportDialog } from './use-import-dialog';

/** Montado na sidebar; cada abertura começa do zero (a key zera o estado) */
export const ImportDialog = memo(() => {
  const open = useImportDialogStore(state => state.isOpen);
  const key = useImportDialogStore(state => state.openCount);
  if (key === 0) return null;
  return <ImportDialogContent key={key} open={open} />;
});

ImportDialog.displayName = 'ImportDialog';

function ImportDialogContent({ open }: { open: boolean }) {
  const state = useImportDialog(open);
  const { result, dismiss, back, submit, chosenCount, isImporting } = state;

  return (
    <Dialog open={open} onOpenChange={next => !next && dismiss()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Importar conexões</DialogTitle>
          <DialogDescription>
            {result
              ? 'Escolha o que entra. Nada é alterado no app de origem.'
              : 'Traga as conexões que você já tem em outro app — sem recadastrar uma a uma.'}
          </DialogDescription>
        </DialogHeader>

        {result ? (
          <ReviewStep
            result={result}
            candidates={state.candidates}
            selected={state.selected}
            allChosen={state.allChosen}
            someChosen={state.someChosen}
            toggle={state.toggle}
            toggleAll={state.toggleAll}
            serverNames={state.serverNames}
            sshCount={state.sshCount}
            hasPasswords={state.hasPasswords}
            keychainCount={state.keychainCount}
            includePasswords={state.includePasswords}
            setIncludePasswords={state.setIncludePasswords}
            keepGroups={state.keepGroups}
            setKeepGroups={state.setKeepGroups}
            groups={state.groups}
            targetGroupId={state.targetGroupId}
            setTargetGroupId={state.setTargetGroupId}
          />
        ) : (
          <SourceStep
            sources={state.sources}
            isDetecting={state.isDetecting}
            detectError={state.detectError}
            isReading={state.isReading}
            readingSource={state.readingSource}
            readSource={state.readSource}
            chooseFile={state.chooseFile}
            urls={state.urls}
            setUrls={state.setUrls}
            readUrls={state.readUrls}
          />
        )}

        <DialogFooter>
          {result ? (
            <>
              <Button
                variant="ghost"
                size="sm"
                className="mr-auto"
                onClick={back}
              >
                Voltar
              </Button>
              <Button variant="ghost" size="sm" onClick={dismiss}>
                Cancelar
              </Button>
              <Button
                size="sm"
                disabled={chosenCount === 0 || isImporting}
                onClick={() => void submit()}
              >
                {isImporting
                  ? 'Importando…'
                  : chosenCount === 1
                    ? 'Importar 1 conexão'
                    : `Importar ${chosenCount} conexões`}
              </Button>
            </>
          ) : (
            <Button variant="ghost" size="sm" onClick={dismiss}>
              Fechar
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
