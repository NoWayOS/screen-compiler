// Synthetic Screen Compiler fixture: pictures (contract v15). A card with a
// photograph covering a fixed box, a heading and a line of text; a badge at
// its own size beside a note; and a photograph contained in a square, the
// card showing on either side. Written for Screen Compiler's tests; not
// taken from any product.
import { Button, Card, Column, Image, Row, Screen, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 16;

type PhotoCardProps = {
  onBook: () => void;
};

export function PhotoCard(props: PhotoCardProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Card gap={GAP} padding={EDGE}>
          <Image
            src="landscape"
            alt="A lake at sunset, hills behind it"
            style={{ width: 320, height: 120, objectFit: "cover" }}
          />
          <Text role="heading">Evening by the lake</Text>
          <Text>Two hours on the water, setting out at six.</Text>
          <Row gap={GAP}>
            <Image src="badge" alt="" />
            <Text>Places left</Text>
          </Row>
          <Button onPress={props.onBook}>Book</Button>
        </Card>
        <Row gap={GAP}>
          <Image src="landscape" alt="The same lake, whole" fit="contain" style={{ width: 120, height: 120 }} />
          <Text>Seen from the north shore.</Text>
        </Row>
      </Column>
    </Screen>
  );
}
