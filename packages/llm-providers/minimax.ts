import { Provider } from "../provider"
import { ProviderID, type ModelID } from "../schema"
import * as AnthropicMessages from "../protocols/anthropic-messages"
import type { AnthropicMessagesModelInput } from "../protocols/anthropic-messages"

export const id = ProviderID.make("minimax")

export type ModelOptions = Omit<AnthropicMessagesModelInput, "id" | "provider" | "baseURL"> & {
  readonly apiKey: string
  readonly baseURL?: string
}

export const routes = [AnthropicMessages.route]

export const model = (id: string | ModelID, options: ModelOptions) => {
  return AnthropicMessages.model({
    ...options,
    id,
    provider: id,
    baseURL: options.baseURL ?? "https://api.minimax.io/anthropic/v1",
  })
}

export const provider = Provider.make({
  id,
  model: (id: string | ModelID, options: ModelOptions) => model(id, options),
})

export const models = {
  "MiniMax-M2.7": {
    id: "MiniMax-M2.7",
    family: "minimax",
    contextLimit: 196608,
    outputLimit: 128000,
    reasoning: true,
    temperature: true,
    toolCall: true,
  },
  "minimax-coding-plan/MiniMax-M2.7": {
    id: "minimax-coding-plan/MiniMax-M2.7",
    family: "minimax",
    contextLimit: 196608,
    outputLimit: 128000,
    reasoning: true,
    temperature: true,
    toolCall: true,
  },
  "minimax/minimax-m2.7": {
    id: "minimax/minimax-m2.7",
    family: "minimax",
    contextLimit: 196608,
    outputLimit: 128000,
    reasoning: true,
    temperature: true,
    toolCall: true,
  },
}