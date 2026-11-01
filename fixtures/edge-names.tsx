// Synthetic Screen Compiler fixture: names at the edges of what converts. A
// callback prop whose second word starts with a digit keeps its `on`, so the
// generated action is a valid identifier; acronyms and ids in different
// styles become snake or screaming case. Written for Screen Compiler's tests.
import { Button, Column, Screen, Text, TextInput } from "nowayos:kit";

type Props = {
  on_2fa: () => void;
  onRetry2: () => void;
  onHTTPCheck: () => void;
};

export function VerifyStep({ on_2fa, onRetry2, onHTTPCheck }: Props) {
  return (
    <Screen>
      <Column gap={8}>
        <Text id="step-title">Enter the code</Text>
        <TextInput id="code_1" placeholder="6-digit code" />
        <Button onPress={on_2fa}>Use an authenticator app</Button>
        <Button onPress={onRetry2}>Send a new code</Button>
        <Button onPress={onHTTPCheck}>Check the connection</Button>
      </Column>
    </Screen>
  );
}
