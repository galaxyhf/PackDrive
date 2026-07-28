# PackDrive

Aplicativo desktop leve para organizar e copiar arquivos de atendimentos para uma pasta local sincronizada pelo Google Drive para computador.

O PackDrive não utiliza a API do Google Drive. Todas as operações acontecem diretamente no sistema de arquivos, e o Google Drive para computador continua responsável pela sincronização com a nuvem.

> A primeira versão é destinada ao Windows. A arquitetura separa a interface React das operações nativas em Rust para permitir suporte futuro ao macOS.

## Sumário

- [Principais recursos](#principais-recursos)
- [Como funciona](#como-funciona)
- [Tecnologias](#tecnologias)
- [Requisitos](#requisitos)
- [Instalação e desenvolvimento](#instalação-e-desenvolvimento)
- [Configuração inicial](#configuração-inicial)
- [Comportamento das transferências](#comportamento-das-transferências)
- [Estrutura do projeto](#estrutura-do-projeto)
- [Comandos disponíveis](#comandos-disponíveis)
- [Validação e testes](#validação-e-testes)
- [Gerando o instalador](#gerando-o-instalador)
- [Privacidade e segurança](#privacidade-e-segurança)
- [Solução de problemas](#solução-de-problemas)
- [Limitações conhecidas](#limitações-conhecidas)

## Principais recursos

- Detecção automática de unidades e pastas comuns do Google Drive no Windows.
- Seleção manual do Google Drive quando a detecção automática não encontra o local correto.
- Envio rápido para pastas no formato `[NUMERO_DO_ATENDIMENTO]`.
- Seleção de arquivos, pastas ou uma combinação dos dois.
- Suporte a arrastar e soltar vários itens sobre a janela.
- Navegador manual de pastas dentro do destino configurado.
- Criação de pastas com validação de nomes inválidos e reservados do Windows.
- Preservação da estrutura de diretórios, incluindo subpastas vazias.
- Cópia em blocos pelo backend Rust, sem carregar arquivos inteiros na memória.
- Progresso geral e do arquivo atual, velocidade, quantidade de itens e tempo estimado.
- Cancelamento seguro após o bloco de escrita atual.
- Arquivos temporários `.uploading` para evitar a sincronização de conteúdo incompleto.
- Políticas para duplicados: renomear, substituir, ignorar ou perguntar.
- Histórico local com abertura do destino, repetição do envio e cópia do caminho.
- Preferências e histórico persistidos localmente com o plugin Store do Tauri.

## Como funciona

### Envio rápido

1. O PackDrive valida o local configurado para o Google Drive.
2. O usuário informa somente o número do atendimento.
3. O aplicativo apresenta a pasta que será criada, por exemplo `[1234567890]`.
4. O usuário seleciona ou arrasta os arquivos e pastas.
5. O backend cria a pasta dentro do destino padrão e copia os itens.
6. Ao concluir, o envio é registrado no histórico.
7. Se a preferência estiver habilitada, a pasta é aberta no Explorador de Arquivos.

Exemplo:

```text
Pasta padrão:
G:\Meu Drive\Atendimentos

Atendimento:
1234567890

Destino:
G:\Meu Drive\Atendimentos\[1234567890]
```

Se a pasta do atendimento já existir, ela não é apagada. Os novos itens são adicionados de acordo com a política de duplicados configurada.

### Navegação manual

Em **Navegar no Drive**, é possível:

- entrar e voltar entre subpastas;
- atualizar a listagem;
- exibir ou ocultar arquivos;
- criar e selecionar uma nova pasta;
- abrir o destino no Explorador;
- enviar itens diretamente para a pasta escolhida.

## Tecnologias

| Camada | Tecnologia |
| --- | --- |
| Desktop | Tauri 2 |
| Interface | React 19, TypeScript e Vite |
| Estilos | Tailwind CSS 4 e CSS |
| Ícones | Lucide React |
| Backend nativo | Rust |
| Seleção de arquivos | Plugin Dialog do Tauri |
| Preferências e histórico | Plugin Store do Tauri |
| Abertura de pastas | Plugin Opener do Tauri |
| Percurso de diretórios | `walkdir` |

## Requisitos

### Para utilizar no Windows

- Windows 10 ou 11;
- Google Drive para computador instalado e conectado;
- acesso de leitura às origens selecionadas;
- acesso de escrita à pasta de destino.

### Para desenvolver

- Node.js `20.19` ou superior, ou `22.12` ou superior;
- npm;
- Rust estável com Cargo;
- Microsoft C++ Build Tools com a carga de trabalho **Desenvolvimento para Desktop com C++**;
- Microsoft Edge WebView2;
- dependências de desenvolvimento do [Tauri 2](https://v2.tauri.app/start/prerequisites/).

O Google Drive para computador não é necessário para compilar o projeto, mas é necessário para testar o fluxo real de envio no Windows.

## Instalação e desenvolvimento

Clone ou abra o repositório e instale as dependências:

```bash
npm install
```

Inicie o aplicativo em modo de desenvolvimento:

```bash
npm run tauri dev
```

Esse comando inicia o Vite na porta `1420`, compila o backend Rust e abre a janela nativa do Tauri.

Para executar apenas a interface no navegador:

```bash
npm run dev
```

Algumas funções dependem do runtime do Tauri e não estarão disponíveis ao executar somente o Vite.

## Configuração inicial

Na primeira execução:

1. O PackDrive procura unidades e caminhos com nomes como `Google Drive`, `Meu Drive`, `My Drive`, `Drive compartilhado` e `Shared drives`.
2. Confirme um local detectado ou use **Alterar localização** para selecionar a pasta manualmente.
3. Selecione uma pasta padrão já existente dentro do Google Drive.
4. O aplicativo valida existência, acesso, permissão de escrita e espaço disponível.

As configurações são armazenadas pelo plugin Store no diretório de dados do aplicativo definido pelo sistema operacional. Nenhuma conta ou credencial do Google é armazenada.

## Comportamento das transferências

### Preservação de estrutura

Ao selecionar uma pasta, o nome da pasta e sua estrutura interna são preservados:

```text
Origem:
C:\Backup\Empresa

Destino:
G:\Meu Drive\Atendimentos\[1234567890]\Empresa
```

### Arquivos duplicados

O comportamento padrão é **renomear automaticamente**:

```text
backup.zip
backup (1).zip
backup (2).zip
```

Também é possível:

- substituir o arquivo existente;
- ignorar o novo arquivo;
- perguntar em cada ocorrência.

### Escrita temporária

Cada arquivo é copiado inicialmente com um sufixo temporário:

```text
arquivo.zip.<id-da-operacao>.uploading
```

Depois que a gravação termina corretamente, o arquivo temporário é renomeado para o nome final. Em caso de erro ou cancelamento, o PackDrive tenta remover o conteúdo parcial.

### Cancelamento

O cancelamento:

- interrompe a operação depois do bloco atual;
- preserva arquivos já concluídos;
- não remove arquivos preexistentes;
- remove arquivos temporários da operação quando possível;
- registra a operação como cancelada no histórico.

## Estrutura do projeto

```text
PackDrive/
├── src/
│   ├── App.tsx                 # Interface e fluxos principais
│   ├── App.css                 # Tema e componentes visuais
│   ├── storage.ts              # Persistência de configurações e histórico
│   ├── types.ts                # Contratos TypeScript
│   └── main.tsx                # Entrada do React
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # Detecção, navegação, validação e cópia
│   │   └── main.rs             # Entrada nativa
│   ├── capabilities/
│   │   └── default.json        # Permissões dos plugins
│   ├── icons/                  # Ícones dos pacotes nativos
│   ├── Cargo.toml              # Dependências Rust
│   └── tauri.conf.json         # Configuração do aplicativo
├── package.json
├── vite.config.ts
└── README.md
```

## Comandos disponíveis

| Comando | Descrição |
| --- | --- |
| `npm run dev` | Inicia somente o servidor Vite |
| `npm run build` | Verifica o TypeScript e gera o frontend de produção |
| `npm run preview` | Serve localmente o frontend gerado |
| `npm run tauri dev` | Executa o aplicativo Tauri em desenvolvimento |
| `npm run tauri build` | Compila e empacota o aplicativo nativo |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Executa os testes do backend |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Executa a análise estática do Rust |

## Validação e testes

Valide o frontend:

```bash
npm run build
```

Formate, teste e analise o backend:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Os testes atuais cobrem as regras de nomes de pastas do Windows e o renomeio automático de arquivos duplicados.

## Gerando o instalador

O pacote do Windows deve ser gerado em uma máquina Windows com os requisitos do Tauri instalados:

```bash
npm run tauri build
```

Os artefatos são produzidos dentro de:

```text
src-tauri\target\release\bundle\
```

Os formatos disponíveis dependem dos targets habilitados e das ferramentas instaladas no ambiente de build.

## Privacidade e segurança

- O PackDrive não acessa a API do Google Drive.
- O aplicativo não solicita login ou token do Google.
- Arquivos não são enviados para um servidor intermediário.
- Configurações e histórico permanecem no computador.
- Origens são lidas em streaming e os dados são gravados diretamente no destino.
- Caminhos de destino são normalizados e precisam permanecer dentro do Drive configurado.
- Nomes com path traversal ou caracteres inválidos do Windows são bloqueados.
- Antes da cópia são verificados destino, permissão de escrita, origens e espaço disponível.

## Solução de problemas

### O Google Drive não foi encontrado

- Confirme que o Google Drive para computador está aberto e conectado.
- Verifique se a unidade aparece no Explorador de Arquivos.
- Abra **Configurações** e use **Alterar localização**.
- Selecione a pasta raiz do Google Drive montado no computador.

### A pasta padrão não é aceita

- A pasta deve existir.
- Ela precisa estar dentro do Google Drive configurado.
- O usuário atual deve possuir permissão de escrita.
- Escolha outra pasta em **Configurações** e execute a validação novamente.

### Um arquivo não foi copiado

- Verifique se a origem ainda existe.
- Confirme que o arquivo não está bloqueado por outro programa.
- Verifique o espaço livre no destino.
- Confira no histórico se a operação terminou com erros.
- Caminhos excessivamente longos podem ser recusados para evitar problemas no Windows.

### O aplicativo não compila no Windows

- Confirme a instalação do Rust com `rustc --version`.
- Confirme a instalação do Node.js com `node --version`.
- Instale o WebView2 e o Microsoft C++ Build Tools.
- Execute `npm install` novamente.
- Consulte os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/).

## Limitações conhecidas

- A primeira versão implementa detecção automática específica para Windows.
- A localização do Drive pode exigir seleção manual em instalações com nomes ou montagens incomuns.
- O PackDrive confirma apenas a cópia para a pasta local; ele não acompanha a conclusão da sincronização com a nuvem.
- O histórico é local ao computador e não é sincronizado entre instalações.
- O aplicativo não resolve conflitos criados posteriormente pelo serviço de sincronização do Google Drive.
