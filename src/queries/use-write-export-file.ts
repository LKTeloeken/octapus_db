import { useMutation } from '@tanstack/react-query';
import { writeExportFile } from '@/api/export';

export interface WriteExportFileVariables {
  path: string;
  contents: string;
}

/** Grava o arquivo exportado; nada em cache muda, então não há invalidação */
export function useWriteExportFile() {
  return useMutation({
    mutationFn: ({ path, contents }: WriteExportFileVariables) =>
      writeExportFile(path, contents),
  });
}
