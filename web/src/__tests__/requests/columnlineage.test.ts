// Copyright 2018-2023 contributors to the Marquez project
// SPDX-License-Identifier: Apache-2.0

import * as requestUtils from '../../store/requests'
import { getColumnLineage } from '../../store/requests/columnlineage'

describe('getColumnLineage', () => {
  let spy: jest.SpyInstance<Promise<any>, [string, requestUtils.IParams, string]>

  beforeEach(() => {
    spy = jest.spyOn(requestUtils, 'genericFetchWrapper').mockImplementation(() => {})
  })

  const queryFor = (url: string) => url.split('?').pop()!.split('&')

  it('single-encodes the nodeId for URI-shaped namespaces', () => {
    getColumnLineage('DATASET', 's3://test-bucket', 'manual-out/', 2)
    const params = queryFor(spy.mock.lastCall[0])
    expect(params).toContain('nodeId=dataset%3As3%3A%2F%2Ftest-bucket%3Amanual-out%2F')
    expect(params).toContain('depth=2')
    expect(params).toContain('withDownstream=true')
  })

  it('single-encodes the nodeId for simple namespaces', () => {
    getColumnLineage('JOB', 'foo', 'bar', 0)
    const params = queryFor(spy.mock.lastCall[0])
    expect(params).toContain('nodeId=job%3Afoo%3Abar')
    expect(params).toContain('depth=0')
    expect(params).toContain('withDownstream=true')
  })
})
