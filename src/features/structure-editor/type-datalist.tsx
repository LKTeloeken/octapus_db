import type { DatabaseType } from '@/api/types/server.types';
import { TYPE_SUGGESTIONS, typeListId } from './type-suggestions';

/** As sugestões de tipo que os campos referenciam com `list` */
export function TypeDatalist({ dbType }: { dbType: DatabaseType }) {
  return (
    <datalist id={typeListId(dbType)}>
      {TYPE_SUGGESTIONS[dbType]?.map(type => <option key={type} value={type} />)}
    </datalist>
  );
}
