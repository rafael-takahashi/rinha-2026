# Rinha 2026

## Algoritmos e Estruturas de Dados

- K-Nearest Neighbors (KNN) - aproximado, K=5
- Índice IVF (Inverted File) - quantizador grosseiro via k-means (nlist=2048), nprobe=8
- Vetores em f16 (meia precisão), layout SoA por célula
- Distância Euclidiana (ao quadrado) com SIMD AVX2 + FMA + F16C e fallback escalar
- KD-Tree exata (kiddo) - implementação anterior, usado como oráculo para validação

## IPC e Redes

- HTTP sobre Unix domain sockets (variável de ambiente SOCKET_PATH)
- Nginx com load balancing round-robin entre duas instâncias da API
- Porta 9999 exposta externamente

## Tecnologias e Dependências

- Rust
- Tokio - runtime assíncrono
- Axum - framework HTTP
- half - codificação f16 dos vetores
- rkyv - desserialização zero-copy (índice IVF serializado no build, lido direto do mmap)
- memmap2 - I/O com arquivo mapeado em memória
- sonic-rs - parsing de JSON de alta performance (com desserialização zero-copy / borrow de strings)
- serde / serde_json - serialização
- kiddo - KD-Tree imutável (oráculo offline)
- flate2 - descompressão gzip (dataset de referência)
- Nginx

## Binários

- `fraud-detection` - servidor HTTP de produção
- `preprocess` - constrói o índice IVF (e a KD-Tree do oráculo) a partir do dataset
- `harness` - replay de queries comparando IVF vs oráculo exato (agreement + throughput)
- `validate` - varredura de nprobe sobre os dados reais
