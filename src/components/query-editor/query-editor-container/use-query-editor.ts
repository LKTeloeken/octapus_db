import { useState } from 'react';

export const useQueryEditor = () => {
  const [selectedQuery, setSelectedQuery] = useState<string>('');

  return {
    selectedQuery,
    setSelectedQuery,
  };
};
