import { describe, expect, it } from 'vitest'
import { refNameError, slugifyBranch } from './validate'
import { basename, dirname, initials, shortSha } from './format'

describe('refNameError', () => {
  it('accepts normal branch names', () => {
    expect(refNameError('feature/login')).toBeNull()
    expect(refNameError('v1.2.0')).toBeNull()
  })
  it('rejects invalid names', () => {
    for (const n of ['', 'a b', '-x', 'a..b', 'a~1', 'x:y', 'a/', 'b.lock', 'c@{1}', 'x\\y']) {
      expect(refNameError(n), n).not.toBeNull()
    }
  })
})

describe('helpers', () => {
  it('slugifies branch names', () => {
    expect(slugifyBranch('Correção do Login!')).toBe('correcao-do-login')
  })
  it('formats paths and names', () => {
    expect(basename('src/a/b.ts')).toBe('b.ts')
    expect(dirname('src/a/b.ts')).toBe('src/a')
    expect(initials('Ana Maria Souza')).toBe('AS')
    expect(shortSha('0123456789abcdef')).toBe('0123456')
  })
})
