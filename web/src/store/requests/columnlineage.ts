import { API_URL } from '../../globals'
import { ColumnLineageGraph } from '../../types/api'
import { JobOrDataset } from '../../types/lineage'

import { generateNodeId } from '../../helpers/nodes'
import { genericFetchWrapper } from './index'

export interface IColumnLineageState {
  columnLineage: ColumnLineageGraph
}

export const getColumnLineage = async (
  nodeType: JobOrDataset,
  namespace: string,
  name: string,
  depth: number
) => {
  const nodeId = generateNodeId(nodeType, namespace, name)
  const url =
    `${API_URL}/column-lineage?nodeId=${encodeURIComponent(nodeId)}` +
    `&depth=${depth}&withDownstream=true`
  return genericFetchWrapper(url, { method: 'GET' }, 'fetchColumnLineage')
}
