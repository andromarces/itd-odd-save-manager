import { describe, expect, it } from 'vitest';

describe('frontend smoke tests', () => {
  it('creates a DOM node in the test environment', () => {
    const element = document.createElement('div');
    element.id = 'smoke-test';
    document.body.appendChild(element);

    expect(document.querySelector('#smoke-test')).not.toBeNull();
  });
});
