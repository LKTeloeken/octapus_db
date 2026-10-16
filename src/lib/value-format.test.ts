import { describe, expect, it } from 'vitest';
import {
  encodeText,
  formatBytes,
  hexDump,
  isValidJson,
  minifyJson,
  parseJsonList,
  prettyJson,
  sniffFormat,
} from './value-format';

// XML e HTML passam pelo DOMParser, que o ambiente `node` dos testes não tem;
// aqui fica o que é texto puro: JSON e a visão binária.

describe('value-format', () => {
  describe('JSON', () => {
    it('reindenta sem passar por JSON.parse: inteiro grande e escapes intactos', () => {
      const raw = '{"id":12345678901234567890,"s":"a\\u00e9\\n"}';
      expect(prettyJson(raw)).toBe(
        '{\n  "id": 12345678901234567890,\n  "s": "a\\u00e9\\n"\n}',
      );
      expect(minifyJson(prettyJson(raw)!)).toBe(raw);
    });

    it('vazios e aninhados', () => {
      expect(prettyJson('{"a":[],"b":{}}')).toBe('{\n  "a": [],\n  "b": {}\n}');
      expect(prettyJson('[1,[2]]')).toBe('[\n  1,\n  [\n    2\n  ]\n]');
    });

    it('texto que não é JSON devolve null', () => {
      expect(isValidJson('{a:1}')).toBe(false);
      expect(prettyJson('{a:1}')).toBeNull();
      expect(minifyJson('[1,')).toBeNull();
    });

    it('lista plana guarda o texto de cada número', () => {
      expect(
        parseJsonList('[1, 99999999999999999999, "x", null, true]'),
      ).toEqual(['1', '99999999999999999999', 'x', null, 'true']);
      expect(parseJsonList('[]')).toEqual([]);
      // Objeto ou array aninhado não é lista plana
      expect(parseJsonList('[{"a":1}]')).toBeNull();
      expect(parseJsonList('{"a":1}')).toBeNull();
    });

    it('adivinha JSON pelo conteúdo, e texto no resto', () => {
      expect(sniffFormat('  {"a":1}')).toBe('json');
      expect(sniffFormat('[1,2]')).toBe('json');
      expect(sniffFormat('{não é json')).toBe('text');
      expect(sniffFormat('olá')).toBe('text');
    });
  });

  describe('binário', () => {
    it('codifica em cada encoding; fora do repertório vira "?"', () => {
      expect(Array.from(encodeText('é', 'utf-8'))).toEqual([0xc3, 0xa9]);
      expect(Array.from(encodeText('é', 'iso-8859-1'))).toEqual([0xe9]);
      expect(Array.from(encodeText('é', 'us-ascii'))).toEqual([0x3f]);
      expect(Array.from(encodeText('A', 'utf-16le'))).toEqual([0x41, 0x00]);
      expect(Array.from(encodeText('A', 'utf-16be'))).toEqual([0x00, 0x41]);
    });

    it('dump com offset, hex em dois blocos de 8 e a coluna ASCII', () => {
      const dump = hexDump(encodeText('Octapus\n', 'utf-8'));
      expect(dump).toBe(
        '00000000  4f 63 74 61 70 75 73 0a ' + ' '.repeat(25) + ' |Octapus.|',
      );
    });

    it('tamanho legível', () => {
      expect(formatBytes(512)).toBe('512 B');
      expect(formatBytes(1536)).toBe('1.5 KB');
      expect(formatBytes(3 * 1024 * 1024)).toBe('3.0 MB');
    });
  });
});
