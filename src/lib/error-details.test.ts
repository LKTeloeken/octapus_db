import { describe, expect, it } from 'vitest';
import { ApiError } from '@/api/client';
import { RustCommand } from '@/api/commands';
import { errorDetails } from './error-details';

describe('errorDetails', () => {
  it('o título vem do código, e a mensagem vai inteira', () => {
    const refused = new ApiError(
      'Connection error: error connecting to server: Connection refused (os error 61)',
      RustCommand.TestServerInput,
      'CONNECTION_ERROR',
    );
    expect(errorDetails(refused)).toEqual({
      code: 'CONNECTION_ERROR',
      title: 'Não foi possível conectar',
      message:
        'Connection error: error connecting to server: Connection refused (os error 61)',
    });
    expect(
      errorDetails(
        new ApiError(
          'Connection timed out',
          RustCommand.TestServerInput,
          'CONNECTION_TIMEOUT',
        ),
      ).title,
    ).toBe('O servidor não respondeu a tempo');
  });

  it('erro sem código (ou nem Error) ganha o título genérico', () => {
    expect(errorDetails(new Error('boom'))).toEqual({
      code: 'UNKNOWN',
      title: 'Algo deu errado',
      message: 'boom',
    });
    expect(errorDetails('texto').message).toBe('texto');
  });
});
