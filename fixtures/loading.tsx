// Synthetic Screen Compiler fixture: a generic loading screen whose parts move
// (contract v23) - a card that fades in when its result shows, a pulsing
// badge and a shimmering placeholder.
// Written for Screen Compiler's tests; not taken from any product.
import { Card, Column, ProgressBar, Screen, Text } from "nowayos:kit";

const GAP = 16;
const EDGE = 32;

interface LoadingProps {
  ready: boolean;
  fetched: number;
}

export default function Loading(props: LoadingProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="title">Fetching your results</Text>
        <ProgressBar value={props.fetched} />
        <Card padding={GAP} style={{ animation: "shimmer 1.5s linear infinite" }}>
          <Text>Loading the first result</Text>
        </Card>
        <Card padding={GAP} style={{ animationName: "pulse", animationDuration: "1.2s", animationTimingFunction: "ease-in-out", animationIterationCount: "infinite", animationDirection: "alternate", animationFillMode: "forwards", animationDelay: "100ms" }}>
          <Text>Nearly there</Text>
        </Card>
        {props.ready && (
          <Card padding={GAP} style={{ transition: "opacity 250ms ease-out 50ms" }}>
            <Text>Your results are ready</Text>
          </Card>
        )}
        {props.ready && (
          <Card padding={GAP} style={{ transitionProperty: "transform", transitionDuration: "0.3s", transitionTimingFunction: "cubic-bezier(0.2, 0, 0, 1)", transitionDelay: "80ms" }}>
            <Text>And sorted</Text>
          </Card>
        )}
      </Column>
    </Screen>
  );
}
