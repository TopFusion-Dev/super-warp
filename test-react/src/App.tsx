import { useState } from 'react'
import './App.css'

interface Message {
  role: 'user' | 'assistant'
  content: string
  model?: string
}

const PROVIDERS = [
  { name: 'OpenAI', color: '#10a37f' },
  { name: 'Anthropic', color: '#d4a574' },
  { name: 'Google', color: '#4285f4' },
  { name: 'MiniMax', color: '#f78166', featured: true },
  { name: 'Deepseek', color: '#0066cc' },
  { name: 'Groq', color: '#eb0029' },
]

const MODELS = [
  { id: 'minimax/minimax-m2.7', name: 'MiniMax M2.7', provider: 'MiniMax' },
  { id: 'minimax-coding-plan/MiniMax-M2.7', name: 'MiniMax Coding Plan', provider: 'MiniMax' },
  { id: 'minimax-cn/minimax-m2.7', name: 'MiniMax CN', provider: 'MiniMax' },
]

function App() {
  const [messages, setMessages] = useState<Message[]>([
    { role: 'assistant', content: 'Welcome to super-warp-terminal! MiniMax M2.7 is ready.', model: 'minimax/minimax-m2.7' }
  ])
  const [input, setInput] = useState('')
  const [selectedModel, setSelectedModel] = useState('minimax/minimax-m2.7')
  const [isLoading, setIsLoading] = useState(false)

  const handleSend = async () => {
    if (!input.trim()) return

    const userMessage: Message = { role: 'user', content: input }
    setMessages(prev => [...prev, userMessage])
    setInput('')
    setIsLoading(true)

    setTimeout(() => {
      const assistantMessage: Message = {
        role: 'assistant',
        content: `This is a simulated response from ${selectedModel}. In production, this would call the MiniMax API at https://api.minimax.io/anthropic/v1 with your MINIMAX_API_KEY.`,
        model: selectedModel
      }
      setMessages(prev => [...prev, assistantMessage])
      setIsLoading(false)
    }, 1500)
  }

  return (
    <div className="app">
      {/* Sidebar */}
      <div className="sidebar">
        <div className="sidebar-header">
          <h2>super-warp</h2>
          <span className="version">v0.1.0</span>
        </div>

        <div className="sidebar-section">
          <button className="sidebar-btn active">Terminal</button>
          <button className="sidebar-btn">AI Agent</button>
          <button className="sidebar-btn">Settings</button>
        </div>

        <div className="sidebar-section">
          <h3>Providers</h3>
          <div className="provider-list">
            {PROVIDERS.map(p => (
              <div
                key={p.name}
                className={`provider-item ${p.featured ? 'featured' : ''}`}
                style={{ borderColor: p.color }}
              >
                <span className="provider-dot" style={{ backgroundColor: p.color }} />
                {p.name}
              </div>
            ))}
          </div>
        </div>

        <div className="sidebar-section">
          <h3>Model</h3>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            className="model-select"
          >
            {MODELS.map(m => (
              <option key={m.id} value={m.id}>{m.name}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Main Content */}
      <div className="main">
        {/* Header */}
        <div className="header">
          <div className="header-left">
            <span className="model-badge">MiniMax M2.7</span>
            <span className="status connected">● Connected</span>
          </div>
          <div className="header-right">
            <span className="context-info">Context: 196,608</span>
          </div>
        </div>

        {/* Terminal Area */}
        <div className="terminal">
          <div className="terminal-tabs">
            <span className="tab active">Terminal</span>
            <span className="tab">AI Chat</span>
          </div>

          <div className="messages">
            {messages.map((msg, i) => (
              <div key={i} className={`message ${msg.role}`}>
                <div className="message-content">{msg.content}</div>
                <div className="message-meta">
                  {msg.role === 'user' ? 'You' : msg.model}
                </div>
              </div>
            ))}
            {isLoading && (
              <div className="message assistant loading">
                <div className="loading-indicator">●</div>
                <span>MiniMax is thinking...</span>
              </div>
            )}
          </div>

          {/* Input Area */}
          <div className="input-area">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleSend()}
              placeholder="Type a message to MiniMax..."
              className="input"
            />
            <button onClick={handleSend} disabled={isLoading} className="send-btn">
              {isLoading ? 'Sending...' : 'Send'}
            </button>
          </div>
        </div>

        {/* Status Bar */}
        <div className="status-bar">
          <span>Model: {selectedModel}</span>
          <span>Provider: MiniMax</span>
          <span>API: api.minimax.io</span>
          <span>Context: 0 / 196,608</span>
        </div>
      </div>
    </div>
  )
}

export default App