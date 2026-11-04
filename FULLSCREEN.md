# Tela cheia do Ani Player

## Anatomia

Cabeçalho com nome e retorno à janela → capa com episódio e progresso → grade de ações → instruções de navegação.

## Variantes e dimensões

A grade aparece no início, na fila de episódios, nas faixas, na fluidez e no Anime4K. Usa três colunas em áreas a partir de 1000 pontos, duas a partir de 560 e uma abaixo disso. Os botões têm altura mínima de 82 pontos, ampliada para 136 em telas largas. O espaçamento entre botões é 16 pontos. A capa ocupa 43% da altura disponível, limitada a 640 pontos.

## Estados

| Estado | Apresentação e comportamento |
| --- | --- |
| Padrão | Superfície CONTROL, nome e descrição |
| Hover | Superfície HOVER pelos controles egui |
| Foco | Borda SAKURA e superfície HOVER |
| Pressionado | Resposta nativa do botão egui |
| Desabilitado | Atenuação nativa e bloqueio de ativação |
| Carregamento | Mensagem de preparação e abertura indisponível até o mpv iniciar |
| Erro | Mensagem textual na área principal com token ERROR |
| Selecionado | Nome com “Ativo” e seleção egui |

Cores e contraste usam os tokens de theme.rs. Geometria da grade usa os valores compartilhados de fullscreen.rs. A renderização segue a adaptação nativa do protocolo do kit, com egui e AccessKit.

## Navegação

F11 abre o menu em tela cheia. Setas e direcional mudam de ação. Enter, Espaço ou A ativam a ação. Esc e B retornam ao início e depois à janela. Durante a reprodução, Esc ou Start abre o menu e pausa o episódio. Continuar assistindo retoma o vídeo.

O foco retorna à ação que abriu o submenu. Itens desabilitados são ignorados pelo direcional. Tab e Shift+Tab seguem os controles nativos, inclusive o botão de retorno. XInput suporta controles compatíveis com Xbox no Windows. Os comandos de um controle mantido pressionado são reconhecidos uma vez a cada pressão.

## Verificação

Dois testes nativos cobrem navegação com itens desabilitados, limites da grade e restauração de foco. O teste integrado de reprodução também foi executado. A captura nativa em 3840 × 2160 foi inspecionada e levou à correção da sobreposição da capa com os botões.

Os verificadores HTML do kit não medem os controles nativos egui. O suporte a leitor de tela usa AccessKit. O percurso com Narrador e o uso de um controle físico seguem sem validação.
